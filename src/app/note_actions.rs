//! Contextual mutations capture durable identities across the two-key operator.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{App, Focus};

impl App {
    pub(super) fn note_key(&mut self, key: KeyEvent) -> Result<bool> {
        let pending = self.note_marks.delete.take();
        if self.selection.is_some()
            || self.vim.prefix_pending()
            || !key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
        {
            return Ok(false);
        }
        if pending.is_some() && key.code == KeyCode::Esc {
            self.status = "Note deletion canceled".into();
            return Ok(true);
        }
        let target = self.cursor_note();
        if self.reader_note_edit_key(key.code)
            && let Some(id) = target
        {
            self.edit_note(id)?;
            return Ok(true);
        }
        match key.code {
            KeyCode::Char('d') if target.is_some() || pending.is_some() => {
                self.note_delete_key(key, target, pending)?;
            }
            KeyCode::Char('D') | KeyCode::Delete if target.is_some() => {
                if key.kind != KeyEventKind::Repeat {
                    self.delete_note(target.context("Select a note")?)?;
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn reader_note_edit_key(&self, code: KeyCode) -> bool {
        if self.focus != Focus::Reader {
            return false;
        }
        match code {
            KeyCode::Enter => true,
            KeyCode::Char('e') => self.cursor_offset() >= self.article_text_end,
            _ => false,
        }
    }

    fn note_delete_key(
        &mut self,
        key: KeyEvent,
        target: Option<i64>,
        pending: Option<(i64, i64)>,
    ) -> Result<()> {
        if key.kind == KeyEventKind::Repeat {
            self.note_marks.delete = pending;
            return Ok(());
        }
        self.vim.clear_input();
        let Some(id) = target else {
            return Ok(());
        };
        let article = self.current().context("Select an article")?.id;
        if pending == Some((article, id)) {
            return self.delete_note(id);
        }
        self.note_marks.delete = Some((article, id));
        self.status = "Note · d again deletes · Esc cancels · u undo".into();
        Ok(())
    }

    pub(super) fn edit_note(&mut self, id: i64) -> Result<()> {
        self.comment_selected = self
            .comments
            .iter()
            .position(|note| note.id == id)
            .context("Note no longer belongs to this article")?;
        self.edit_comment(true)
    }

    pub(super) fn edit_context_note(&mut self) -> Result<()> {
        if self.notebook.is_some() {
            return self.edit_notebook_note();
        }
        if let Some(id) = self.cursor_note() {
            return self.edit_note(id);
        }
        self.edit_comment(true)
    }

    pub(super) fn delete_note(&mut self, id: i64) -> Result<()> {
        let article = self.current().context("Select an article")?.id;
        self.delete_captured_note(article, id)
    }

    pub(super) fn delete_captured_note(
        &mut self,
        article: i64,
        id: i64,
    ) -> Result<()> {
        if self
            .pending_note_deletes
            .values()
            .any(|pending| *pending == id)
        {
            self.status = "This note's deletion is already pending".into();
            return Ok(());
        }
        if let Some(writer) = &mut self.personal_writes {
            let ticket = writer.submit(super::personal_tasks::Task::Edit(
                crate::app::NoteEdit::Delete { article, note: id },
            ))?;
            self.pending_note_deletes.insert(ticket, id);
            self.status =
                "Deleting note… · navigation remains available".into();
            return Ok(());
        }
        let row = self.reader_cursor;
        self.store.delete_comment(article, id)?;
        self.refresh_notes(article)?;
        if self.current().is_some_and(|item| item.id == article) {
            self.reader_cursor = row;
            self.clamp_cursor();
            self.ensure_cursor_visible();
        }
        self.status =
            "Note deleted · u restores the note and study material".into();
        Ok(())
    }
}
