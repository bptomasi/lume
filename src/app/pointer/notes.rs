//! Hover and popup clicks share the reader's painted gutter and text geometry.

use crossterm::event::MouseEvent;
use ratatui::layout::Position;

use crate::{model::Pane, presentation::geometry};

use super::App;

impl App {
    pub(super) fn hover_note(&mut self, event: MouseEvent) {
        if self.pointer.gesture.is_some() {
            return;
        }
        let position = Position::new(event.column, event.row);
        if self.note_popup_at(position).is_some() {
            return;
        }
        let id = self.reader_note_at_pointer(event, false);
        if self.reader.hovered_note().map(|(old, _)| old) != id {
            self.reader.hover_note(id.map(|id| (id, position)));
        }
    }

    pub(super) fn note_popup_at(&self, position: Position) -> Option<i64> {
        let (id, anchor) = self.reader.hovered_note()?;
        geometry::note_preview_area(self.layout.screen(), anchor)
            .contains(position)
            .then_some(id)
    }

    pub(super) fn reader_note_at_pointer(
        &self,
        event: MouseEvent,
        gutter_only: bool,
    ) -> Option<i64> {
        if self.notebook().is_some()
            || self.article_preparing()
            || self.reader_stale()
        {
            return None;
        }
        let area = geometry::workspace_areas(
            self.layout.screen(),
            self.layout.focus(),
            self.layout.workspace(),
        )
        .reader?;
        let content =
            geometry::workspace_reader_content(area, self.layout.workspace());
        if event.row < content.y || event.row >= content.bottom() {
            return None;
        }
        let row = self.reader.first_row() + usize::from(event.row - content.y);
        if event.column == content.x.saturating_sub(1) {
            return self.reader_note_markers(row).first().copied();
        }
        if gutter_only
            || !content.contains(Position::new(event.column, event.row))
        {
            return None;
        }
        let offset = self.pointer_offset(content, event)?;
        self.note_at_offset(offset)
    }

    pub(super) fn note_pointer_down(
        &mut self,
        event: MouseEvent,
    ) -> anyhow::Result<bool> {
        self.reader.arm_note_delete(None);
        let position = Position::new(event.column, event.row);
        let Some(id) = self
            .note_popup_at(position)
            .or_else(|| self.reader_note_at_pointer(event, true))
        else {
            return Ok(false);
        };
        self.cancel_pointer();
        self.layout.focus_pane(Pane::Reader);
        self.edit_note(id)?;
        Ok(true)
    }
}
