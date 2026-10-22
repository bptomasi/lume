//! Pane-local gestures own hit testing, temporary widths and captured selection.
//!
//! Only completed layout gestures persist. Cancellation restores provisional
//! widths. Source changes cancel text capture; resize cancels every gesture.
//! Selection uses readable offsets rather than framebuffer text, so another
//! pane cannot enter a quotation or yank.

mod gestures;
mod images;
mod modal;
mod notes;
mod reorder;
mod splits;

use anyhow::Result;
use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use crate::{
    model::{ContentSource, Pane, Workspace},
    ui,
};

use super::{Action, App, Mode};

#[derive(Clone, Copy)]
struct Widths {
    library: u16,
    articles: u16,
    annotations: u16,
}

impl Widths {
    fn capture(workspace: &Workspace) -> Self {
        Self {
            library: workspace.library_width,
            articles: workspace.article_width,
            annotations: workspace.annotation_width,
        }
    }

    fn restore(self, workspace: &mut Workspace) {
        workspace.library_width = self.library;
        workspace.article_width = self.articles;
        workspace.annotation_width = self.annotations;
    }
}

enum Gesture {
    SplitResize {
        boundary: ui::SplitBoundary,
        grab: Position,
        before: crate::model::PaneLayout,
    },
    Resize(ResizeCapture),
    Reorder(reorder::MoveCapture),
    Select(SelectionCapture),
}

struct ResizeCapture {
    left: Pane,
    right: Pane,
    column: u16,
    before: Widths,
}

struct SelectionCapture {
    article: i64,
    source: ContentSource,
    revision: i64,
    anchor: usize,
    moved: bool,
}

/// Transient source and candidate destination; saved order changes on release.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PanelDrag {
    /// Pane being moved with its title or corner grip.
    pub pane: Pane,
    /// Insertion candidate from the moving footprint or an explicit top-tab hit.
    pub target: Option<Pane>,
    /// Explicit split edge; None preserves column insertion semantics.
    pub edge: Option<crate::model::Dock>,
}

#[derive(Default)]
pub(super) struct Pointer {
    gesture: Option<Gesture>,
    pub(super) seen: bool,
}

impl App {
    pub(super) fn cancel_pointer(&mut self) {
        self.note_marks.hover = None;
        self.pane_flight = None;
        self.pane_neighbors = None;
        match self.pointer.gesture.take() {
            Some(Gesture::SplitResize { before, .. }) => {
                self.workspace.layout = Some(before);
                self.resize_workspace(self.screen);
                self.status = "Panel resize canceled".into();
            }
            Some(Gesture::Resize(capture)) => {
                capture.before.restore(&mut self.workspace);
                self.resize_workspace(self.screen);
                self.status = "Panel resize canceled".into();
            }
            Some(Gesture::Reorder(_)) => {
                self.status = "Panel move canceled".into();
            }
            _ => {}
        }
    }

    pub(super) fn cancel_reader_pointer(&mut self) {
        if matches!(self.pointer.gesture, Some(Gesture::Select(_))) {
            self.cancel_pointer();
        }
    }

    pub(super) fn capture_drag_frame(
        &mut self,
        frame: &ratatui::buffer::Buffer,
    ) {
        if let Some(Gesture::Reorder(capture)) = &mut self.pointer.gesture {
            capture.capture(frame);
        }
    }

    pub(super) fn floating_drag(&self) -> Option<super::FloatingPane<'_>> {
        match &self.pointer.gesture {
            Some(Gesture::Reorder(capture)) => capture.floating(),
            _ => None,
        }
    }

    pub(super) fn drag_neighbors(
        &self,
    ) -> Option<&super::pane_motion::Neighbors> {
        match &self.pointer.gesture {
            Some(Gesture::Reorder(capture)) => capture.neighbors.as_ref(),
            _ => None,
        }
    }

    pub(super) fn tick_drag_neighbors(
        &mut self,
        now: std::time::Instant,
    ) -> bool {
        match &mut self.pointer.gesture {
            Some(Gesture::Reorder(capture)) => capture
                .neighbors
                .as_mut()
                .is_some_and(|motion| motion.tick(now)),
            _ => false,
        }
    }

    /// Read the active pane-move preview without changing saved layout.
    pub fn panel_drag(&self) -> Option<PanelDrag> {
        match &self.pointer.gesture {
            Some(Gesture::Reorder(capture)) => Some(capture.preview),
            _ => None,
        }
    }

    /// Dispatch a reported mouse event against the most recent workspace size.
    ///
    /// # Errors
    /// Returns navigation, annotation or completed-layout persistence failures.
    pub fn mouse(&mut self, event: MouseEvent) -> Result<Action> {
        self.pointer.seen = true;
        if matches!(self.mode, Mode::Web) && self.workspace.mouse {
            return self.web_mouse(event);
        }
        if !self.workspace.mouse
            || !event.modifiers.is_empty()
            || self.screen.width < 24
            || self.screen.height < 10
        {
            self.cancel_pointer();
            return Ok(Action::None);
        }
        if !matches!(self.mode, Mode::Normal) {
            return self.modal_pointer(event);
        }
        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.pointer_down(event)?
            }
            MouseEventKind::Drag(MouseButton::Left) => self.pointer_drag(event),
            MouseEventKind::Up(MouseButton::Left) => self.pointer_up(event)?,
            MouseEventKind::ScrollDown | MouseEventKind::ScrollUp => {
                self.pointer_scroll(event, 1)?;
            }
            MouseEventKind::ScrollLeft | MouseEventKind::ScrollRight => {
                self.pointer_horizontal(event);
            }
            MouseEventKind::Moved => self.hover_note(event),
            _ => {}
        }
        Ok(Action::None)
    }

    fn pointer_horizontal(&mut self, event: MouseEvent) {
        self.note_marks.hover = None;
        self.note_marks.delete = None;
        if self.panel_drag().is_some() {
            return;
        }
        let Some(reader) =
            ui::workspace_areas(self.screen, self.focus, &self.workspace)
                .reader
        else {
            return;
        };
        let content = ui::workspace_reader_content(reader, &self.workspace);
        if !content.contains(Position::new(event.column, event.row)) {
            return;
        }
        self.pane_flight = None;
        self.pane_neighbors = None;
        let row = self.scroll + usize::from(event.row - content.y);
        self.scroll_horizontal_row(
            row,
            event.kind == MouseEventKind::ScrollRight,
        );
    }

    fn pointer_down(&mut self, event: MouseEvent) -> Result<()> {
        if self.note_pointer_down(event)? {
            return Ok(());
        }
        self.cancel_pointer();
        let position = Position::new(event.column, event.row);
        if !self.workspace.fullscreen {
            for (pane, button) in ui::pane_buttons(self.screen, &self.workspace)
            {
                if button.contains(position) {
                    return self.toggle_pane(pane);
                }
            }
        }
        let areas =
            ui::workspace_areas(self.screen, self.focus, &self.workspace);
        if self.begin_resize(&areas, event) {
            return Ok(());
        }
        for (pane, area) in areas.ordered() {
            if !area.contains(position) {
                continue;
            }
            if event.row == area.y
                && !ui::pane_close_rect(area)
                    .is_some_and(|rect| rect.contains(position))
            {
                self.begin_reorder(pane, &areas, event);
                return Ok(());
            }
            self.focus = pane;
            self.selection = None;
            self.vim.clear_input();
            if ui::pane_close_rect(area)
                .is_some_and(|rect| rect.contains(position))
            {
                return self.toggle_pane(pane);
            }
            return self.click_pane(pane, area, event);
        }
        Ok(())
    }

    fn click_pane(
        &mut self,
        pane: Pane,
        area: Rect,
        event: MouseEvent,
    ) -> Result<()> {
        if self.notebook.is_some() {
            return self.click_notebook(pane, area, event);
        }
        if self.reader_stale()
            && matches!(pane, Pane::Reader | Pane::Annotations)
        {
            return Ok(());
        }
        match pane {
            Pane::Library => {
                if let Some(index) = ui::navigation_at(self, area, event.row) {
                    let view = self.navigation[index].view.clone();
                    if view == super::View::Annotated {
                        return self.open_notebook("", None);
                    }
                    self.set_view(view)?;
                    if let super::View::Group(_) = self.view {
                        self.toggle_navigation_group("")?;
                    }
                }
            }
            Pane::Articles => self.click_article(area, event)?,
            Pane::Reader => {
                if !self.open_pointer_image(area, event) {
                    self.begin_selection(area, event);
                }
            }
            Pane::Annotations => {
                if let Some(index) = ui::annotation_at(self, area, event.row) {
                    self.comment_selected =
                        self.annotations[index].comment_index;
                    self.jump_annotation(index)?;
                }
            }
        }
        self.resize_workspace(self.screen);
        Ok(())
    }

    fn click_article(&mut self, area: Rect, event: MouseEvent) -> Result<()> {
        if let Some(index) = ui::article_at(self, area, event.row) {
            self.save_reading_position()?;
            self.selected = index;
            self.load_current_deferred()?;
            self.open_reader()?;
        }
        Ok(())
    }

    pub(super) fn pointer_scroll(
        &mut self,
        event: MouseEvent,
        amount: usize,
    ) -> Result<()> {
        self.pointer.seen = true;
        self.note_marks.delete = None;
        if !self.workspace.mouse
            || !event.modifiers.is_empty()
            || self.screen.width < 24
            || self.screen.height < 10
        {
            return Ok(());
        }
        if !matches!(self.mode, Mode::Normal) {
            return self.scroll_modal(event, amount);
        }
        if self.panel_drag().is_some() {
            return Ok(());
        }
        self.pane_flight = None;
        self.pane_neighbors = None;
        self.note_marks.hover = None;
        let areas =
            ui::workspace_areas(self.screen, self.focus, &self.workspace);
        let target = areas.ordered().into_iter().find(|(_, area)| {
            area.contains(Position::new(event.column, event.row))
        });
        if let Some((pane, _)) = target {
            let pane =
                if matches!(self.pointer.gesture, Some(Gesture::Select(_))) {
                    Pane::Reader
                } else {
                    pane
                };
            self.focus = pane;
            self.scroll_focused_pane(event, amount)?;
        }
        self.resize_workspace(self.screen);
        Ok(())
    }
    fn scroll_focused_pane(
        &mut self,
        event: MouseEvent,
        amount: usize,
    ) -> Result<()> {
        let forward = event.kind == MouseEventKind::ScrollDown;
        if self.notebook.is_some() {
            self.move_notebook(forward, amount)?;
        } else if self.focus == Pane::Reader {
            self.scroll_reader(forward, amount);
        } else {
            self.move_selection(forward, amount)?;
        }
        Ok(())
    }
}
