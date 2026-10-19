//! Notebook gestures use note rows, never the underlying article hit map.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEventKind, MouseEvent};
use ratatui::layout::Rect;

use super::super::{App, Focus, Mode};

impl super::Notebook {
    /// Visible note cards, shared by painting and pointer hit testing.
    pub fn window(&self, height: u16) -> (usize, usize) {
        let capacity = usize::from(height / 5).max(1);
        let first = self
            .selected
            .saturating_sub(capacity / 2)
            .min(self.page.notes.len().saturating_sub(capacity));
        (first, capacity)
    }

    /// Visible collection offset, independent of the note search page.
    pub fn collection_start(&self, height: u16) -> usize {
        self.collection
            .saturating_sub(usize::from(height).saturating_sub(1))
    }
}

impl App {
    pub(in crate::app) fn click_notebook(
        &mut self,
        pane: Focus,
        area: Rect,
        event: MouseEvent,
    ) -> Result<()> {
        let notebook = self.notebook.as_mut().context("Open Notebook first")?;
        let row = usize::from(event.row.saturating_sub(area.y + 1));
        match pane {
            Focus::Library => {
                notebook.collection = (notebook
                    .collection_start(area.height.saturating_sub(2))
                    + row)
                    .min(notebook.collections.len() + 3);
                self.choose_note_collection()?;
                self.focus = Focus::Library;
            }
            Focus::Articles => {
                let (first, _) = notebook.window(area.height.saturating_sub(2));
                notebook.selected = (first + row / 5)
                    .min(notebook.page.notes.len().saturating_sub(1));
                notebook.read_context();
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
            KeyCode::Char('p') => self.change_note_metadata("note-pin", "")?,
            KeyCode::Char('t') => self.open_note_tags("")?,
            KeyCode::Char('T') => self.show_note_kinds()?,
            KeyCode::Char('r') => {
                self.change_note_metadata("note-status-toggle", "")?
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
        let notebook = self.notebook.as_mut().context("Open Notebook first")?;
        let note = notebook.current().context("Select a note first")?;
        let (id, article) = (note.id, note.article);
        if notebook.pending_delete.take() != Some(id) {
            notebook.pending_delete = Some(id);
            self.status = "Note · d again deletes · u undo".into();
            return Ok(());
        }
        self.delete_captured_note(article, id)
    }

    fn mark_notebook_note(&mut self) -> Result<()> {
        let notebook = self.notebook.as_mut().context("Open Notebook first")?;
        let id = notebook.current().context("Select a note")?.id;
        if !notebook.marked.remove(&id) {
            ensure!(
                notebook.marked.len() < 20,
                "Select at most 20 notes per synthesis"
            );
            notebook.marked.insert(id);
        }
        Ok(())
    }
}
