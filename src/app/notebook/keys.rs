//! Familiar list keys remain local to the focused notebook pane.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::super::{Action, App, Focus, InputKind, Mode};

impl App {
    pub(in crate::app) fn notebook_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Action> {
        if let Some(action) = self.normal_prefix_action(key)? {
            return Ok(action);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return self.notebook_control(key.code);
        }
        if key.code != KeyCode::Char('d')
            && let Some(notebook) = &mut self.notebook
        {
            notebook.pending_delete = None;
        }
        let previous_g = std::mem::take(&mut self.pending_g);
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.close_notebook(),
            KeyCode::Char(':') => {
                self.begin_input(InputKind::Command, String::new())
            }
            KeyCode::Char('/') => self.begin_note_search(),
            KeyCode::Tab => {
                self.change_focus(true);
            }
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char('y' | 'Y') => {
                return self.copy_notebook_note(key.code);
            }
            KeyCode::Char('e') => self.edit_notebook_note()?,
            KeyCode::Enter => self.activate_notebook_pane()?,
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_notebook(true, 1)?
            }
            KeyCode::Char('k') | KeyCode::Up => self.move_notebook(false, 1)?,
            KeyCode::Char('g') if previous_g => {
                self.move_notebook(false, usize::MAX)?;
            }
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Char('G') => self.move_notebook(true, usize::MAX)?,
            code => self.notebook_personal_key(code, key.kind)?,
        }
        Ok(Action::None)
    }

    fn copy_notebook_note(&self, code: KeyCode) -> Result<Action> {
        let context = self
            .notebook
            .as_ref()
            .and_then(|book| book.current_context())
            .context("Wait for the selected note's context")?;
        let text = if code == KeyCode::Char('Y') {
            &context.note.comment.quote
        } else {
            &context.note.comment.body
        };
        anyhow::ensure!(!text.is_empty(), "This note has no text to copy");
        Ok(Action::Copy(text.clone()))
    }

    fn activate_notebook_pane(&mut self) -> Result<()> {
        match self.focus {
            Focus::Library => self.choose_note_collection(),
            Focus::Annotations => self.open_notebook_link(),
            _ => self.notebook_source(),
        }
    }

    fn open_notebook_link(&mut self) -> Result<()> {
        let notebook = self.notebook.as_ref().context("Open Notebook first")?;
        let context = notebook.current_context().context("Wait for context")?;
        let note = context
            .backlinks
            .iter()
            .chain(&context.references)
            .nth(notebook.link_selected)
            .context("No note references yet")?;
        self.apply_note_action(super::NoteAction::Open(note.id))?;
        Ok(())
    }

    pub(in crate::app) fn begin_global_note_search(&mut self) -> Result<()> {
        if self.notebook.is_none() {
            self.open_notebook("", None)?;
        }
        self.begin_note_search();
        Ok(())
    }

    pub(in crate::app) fn begin_note_search(&mut self) {
        let query = self
            .notebook
            .as_ref()
            .map_or_else(String::new, |notebook| notebook.query.clone());
        self.begin_input(InputKind::NoteSearch, query);
    }

    fn notebook_control(&mut self, code: KeyCode) -> Result<Action> {
        match code {
            KeyCode::Char('d') => self.move_notebook(true, 10)?,
            KeyCode::Char('u') => self.move_notebook(false, 10)?,
            KeyCode::Char('o') => {
                if let Some(notebook) = &mut self.notebook
                    && let Some((query, offset, selected)) =
                        notebook.history.pop()
                {
                    notebook.query = query;
                    notebook.offset = offset;
                    notebook.selected = selected;
                    notebook.refresh()?;
                }
            }
            _ => {}
        }
        Ok(Action::None)
    }

    pub(in crate::app) fn move_notebook(
        &mut self,
        forward: bool,
        distance: usize,
    ) -> Result<()> {
        let notebook = self.notebook.as_mut().context("Open Notebook first")?;
        notebook.pending_delete = None;
        if self.focus == Focus::Annotations {
            let count = notebook.current_context().map_or(0, |context| {
                context.backlinks.len() + context.references.len()
            });
            notebook.link_selected =
                shifted(notebook.link_selected, count, forward, distance);
            return Ok(());
        }
        if self.focus == Focus::Reader {
            let height = crate::ui::workspace_areas(
                self.screen,
                self.focus,
                &self.workspace,
            )
            .reader
            .map_or(1, |area| area.height);
            let limit = crate::ui::notebook_scroll_limit(notebook, height);
            notebook.scroll =
                shifted(notebook.scroll, limit + 1, forward, distance);
            return Ok(());
        }
        if self.focus == Focus::Library {
            notebook.collection = shifted(
                notebook.collection,
                notebook.collections.len() + 4,
                forward,
                distance,
            );
            return Ok(());
        }
        notebook.move_note_rows(forward, distance)
    }

    pub(super) fn choose_note_collection(&mut self) -> Result<()> {
        let notebook = self.notebook.as_ref().context("Open Notebook first")?;
        let query = match notebook.collection {
            0 => "".to_owned(),
            1 => "is:pinned".into(),
            2 => "type:question status:open".into(),
            3 => "type:todo status:open".into(),
            index => notebook
                .collections
                .get(index - 4)
                .context("Select a collection")?
                .query
                .clone(),
        };
        self.set_notebook_query(&query)?;
        self.focus = Focus::Articles;
        Ok(())
    }
}

fn shifted(
    position: usize,
    count: usize,
    forward: bool,
    distance: usize,
) -> usize {
    if forward {
        return position
            .saturating_add(distance)
            .min(count.saturating_sub(1));
    }
    position.saturating_sub(distance)
}

impl super::Notebook {
    fn move_note_rows(&mut self, forward: bool, distance: usize) -> Result<()> {
        let position = self.offset + self.selected;
        let next = shifted(position, self.page.total, forward, distance);
        let offset = next / 100 * 100;
        if offset == self.offset {
            self.selected = next.saturating_sub(offset);
            self.read_context();
        } else {
            self.offset = offset;
            self.selected = next - offset;
            self.refresh()?;
        }
        self.pending_delete = None;
        Ok(())
    }
}
