//! Captured gestures remain bounded to their original pane and source.

use crossterm::event::MouseEvent;
use ratatui::layout::{Position, Rect};

use crate::{
    model::{Pane, Workspace},
    reader::cursor,
    ui,
};

use super::{App, Gesture, SelectionCapture, Widths};

impl App {
    pub(super) fn begin_resize(
        &mut self,
        areas: &ui::Areas,
        event: MouseEvent,
    ) -> bool {
        if self.workspace.layout.is_some() {
            return self.begin_split_resize(areas, event);
        }
        let panes = areas.ordered();
        for pair in panes.windows(2) {
            let (left, area) = pair[0];
            let (right, next) = pair[1];
            if event.row <= area.y
                || event.row >= area.bottom().saturating_sub(1)
            {
                continue;
            }
            if event.column < next.x.saturating_sub(1) || event.column > next.x
            {
                continue;
            }
            self.pointer.gesture =
                Some(Gesture::Resize(super::ResizeCapture {
                    left,
                    right,
                    column: event.column,
                    before: Widths::capture(&self.workspace),
                }));
            return true;
        }
        false
    }

    pub(super) fn begin_selection(&mut self, area: Rect, event: MouseEvent) {
        if self.article_preparing() || self.reader_text.is_empty() {
            return;
        }
        let content = ui::workspace_reader_content(area, &self.workspace);
        if !content.contains(Position::new(event.column, event.row)) {
            return;
        }
        let Some(article) = self.current() else {
            return;
        };
        let Some(anchor) = self.pointer_offset(content, event) else {
            return;
        };
        self.pointer.gesture = Some(Gesture::Select(SelectionCapture {
            article: article.id,
            source: self.content_source,
            revision: self.content_revision,
            anchor,
            moved: false,
        }));
        self.selection = Some(anchor);
        self.linewise = false;
        self.place_cursor(anchor);
        self.ensure_cursor_visible();
    }

    pub(super) fn pointer_drag(&mut self, event: MouseEvent) {
        let Some(gesture) = self.pointer.gesture.take() else {
            return;
        };
        self.pointer.gesture = match gesture {
            Gesture::SplitResize {
                boundary,
                grab,
                before,
            } => {
                self.drag_split(&boundary, grab, &before, event);
                Some(Gesture::SplitResize {
                    boundary,
                    grab,
                    before,
                })
            }
            Gesture::Resize(capture) => {
                self.drag_divider(
                    capture.left,
                    capture.right,
                    capture.column,
                    capture.before,
                    event.column,
                );
                Some(Gesture::Resize(capture))
            }
            Gesture::Reorder(mut capture) => {
                capture.update(event);
                self.status = capture.hint();
                Some(Gesture::Reorder(capture))
            }
            Gesture::Select(mut capture) => {
                if !self.drag_selection(&mut capture, event) {
                    return;
                }
                Some(Gesture::Select(capture))
            }
        };
    }

    fn drag_divider(
        &mut self,
        left: Pane,
        right: Pane,
        column: u16,
        before: Widths,
        next_column: u16,
    ) {
        before.restore(&mut self.workspace);
        let requested = i32::from(next_column) - i32::from(column);
        let delta = divider_delta(&self.workspace, left, right, requested);
        self.workspace.resize_pane(left, delta);
        self.workspace.resize_pane(right, -delta);
        self.resize_workspace(self.screen);
        self.status = "Resizing panes · release saves · Esc cancels".into();
    }

    pub(super) fn pointer_up(
        &mut self,
        event: MouseEvent,
    ) -> anyhow::Result<()> {
        self.pointer_drag(event);
        match self.pointer.gesture.take() {
            Some(Gesture::Resize(_) | Gesture::SplitResize { .. }) => {
                self.commit_panel_layout()?;
            }
            Some(Gesture::Reorder(capture)) => {
                self.complete_reorder(capture)?
            }
            Some(Gesture::Select(capture)) => {
                if !capture.moved {
                    self.selection = None;
                }
                self.status =
                    "Reader selection · y yank · s highlight · n annotate"
                        .into();
            }
            None => {}
        }
        Ok(())
    }

    fn drag_selection(
        &mut self,
        capture: &mut super::SelectionCapture,
        event: MouseEvent,
    ) -> bool {
        if self.current().is_none_or(|item| item.id != capture.article)
            || self.content_source != capture.source
            || self.content_revision != capture.revision
        {
            return false;
        }
        let changed = self.extend_pointer_selection(event);
        capture.moved |= changed.is_some_and(|offset| offset != capture.anchor);
        true
    }

    fn extend_pointer_selection(&mut self, event: MouseEvent) -> Option<usize> {
        let reader =
            ui::workspace_areas(self.screen, self.focus, &self.workspace)
                .reader?;
        let content = ui::workspace_reader_content(reader, &self.workspace);
        if event.row < content.y {
            self.scroll_reader(false, 1);
        } else if event.row >= content.bottom() {
            self.scroll_reader(true, 1);
        }
        let offset = self.pointer_offset(content, event)?;
        self.place_cursor(offset);
        self.ensure_cursor_visible();
        Some(offset)
    }

    fn pointer_offset(
        &self,
        content: Rect,
        event: MouseEvent,
    ) -> Option<usize> {
        if content.is_empty() {
            return None;
        }
        let row = event.row.clamp(content.y, content.bottom() - 1);
        let index = self.scroll + usize::from(row - content.y);
        let text = self.reader_lines.get(index)?;
        let column = event.column.clamp(content.x, content.right() - 1);
        let goal = usize::from(column - content.x)
            + if self.code_lines.get(index) == Some(&true) {
                self.horizontal
            } else {
                0
            };
        let byte = cursor::byte_at(text, goal);
        Some(self.reader_ranges.get(index)?.start + byte)
    }
}

fn divider_delta(
    workspace: &Workspace,
    left: Pane,
    right: Pane,
    requested: i32,
) -> i32 {
    let mut delta = requested;
    for (pane, sign) in [(left, 1), (right, -1)] {
        if let Some(width) = workspace.pane_width(pane) {
            let minimum = if pane == Pane::Library { 16 } else { 24 };
            let maximum = if pane == Pane::Library { 48 } else { 80 };
            let current = i32::from(width);
            let (low, high) = if sign == 1 {
                (minimum - current, maximum - current)
            } else {
                (current - maximum, current - minimum)
            };
            delta = delta.clamp(low, high);
        }
    }
    delta
}
