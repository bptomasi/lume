//! Notebook gestures use note rows, never the underlying article hit map.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEventKind, MouseEvent};
use ratatui::layout::Rect;

use super::super::{App, Focus, Mode};
use crate::model::NoteChange;

impl App {
    pub(in crate::app) fn click_notebook(
        &mut self,
        pane: Focus,
        area: Rect,
        event: MouseEvent,
    ) -> Result<()> {
        let notebook = self
            .notebooks
            .active
            .as_mut()
            .context("Open Notebook first")?;
        let row = usize::from(event.row.saturating_sub(area.y + 1));
        match pane {
            Focus::Library => {
                notebook.click_collection(row, area.height.saturating_sub(2));
                self.choose_note_collection()?;
                self.focus = Focus::Library;
            }
            Focus::Articles => {
                notebook.click_note(row, area.height.saturating_sub(2));
            }
            Focus::Annotations => self.show_note_references()?,
            Focus::Reader => {}
        }
        Ok(())
    }

    pub(super) fn notebook_personal_key(
        &mut self,
        code: KeyCode,
        kind: KeyEventKind,
    ) -> Result<()> {
        match code {
            KeyCode::Char('p') => {
                self.change_note_metadata(NoteChange::TogglePin)?
            }
            KeyCode::Char('t') => self.open_note_tags("")?,
            KeyCode::Char('T') => self.show_note_kinds()?,
            KeyCode::Char('r') => {
                self.change_note_metadata(NoteChange::ToggleResolved)?
            }
            KeyCode::Char('u') => self.undo()?,
            KeyCode::Char('d') if kind != KeyEventKind::Repeat => {
                self.notebook_delete()?;
            }
            KeyCode::Char('v') => self.mark_notebook_note()?,
            KeyCode::Char('s') => self.synthesize_notes()?,
            KeyCode::Char('b') => self.show_note_references()?,
            KeyCode::Char(' ') => self.mode = Mode::Leader,
            _ => {}
        }
        Ok(())
    }

    fn notebook_delete(&mut self) -> Result<()> {
        let notebook = self
            .notebooks
            .active
            .as_mut()
            .context("Open Notebook first")?;
        let Some((article, id)) = notebook.confirm_delete()? else {
            self.status = "Note · d again deletes · u undo".into();
            return Ok(());
        };
        self.delete_captured_note(article, id)
    }

    fn mark_notebook_note(&mut self) -> Result<()> {
        let notebook = self
            .notebooks
            .active
            .as_mut()
            .context("Open Notebook first")?;
        notebook.toggle_mark()
    }
}

impl super::Notebook {
    fn click_collection(&mut self, row: usize, height: u16) {
        self.collection = (self.view().collection_start(height) + row)
            .min(self.collections.len() + 3);
    }

    fn click_note(&mut self, row: usize, height: u16) {
        let (first, _) = self.view().window(height);
        self.selected =
            (first + row / 5).min(self.page.notes.len().saturating_sub(1));
        self.read_context();
    }

    fn confirm_delete(&mut self) -> Result<Option<(i64, i64)>> {
        let note = self.current().context("Select a note first")?;
        let (id, article) = (note.id, note.article);
        if self.pending_delete.take() != Some(id) {
            self.pending_delete = Some(id);
            return Ok(None);
        }
        Ok(Some((article, id)))
    }

    fn toggle_mark(&mut self) -> Result<()> {
        let id = self.current().context("Select a note")?.id;
        if !self.marked.remove(&id) {
            ensure!(
                self.marked.len() < 20,
                "Select at most 20 notes per synthesis"
            );
            self.marked.insert(id);
        }
        Ok(())
    }
}
