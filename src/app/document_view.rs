//! Scrollable read-only inspections share viewport mechanics, never mutation.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use super::{Action, App, ChoiceAction, Mode};

/// Cached text viewport used for previews and wide technical tables.
#[derive(Clone, Debug)]
pub struct DocumentView {
    /// Context and controls displayed in the frame.
    pub title: String,
    /// Terminal-safe, unwrapped lines prepared before drawing.
    pub lines: Vec<String>,
    /// First visible row.
    pub row: usize,
    /// First visible terminal column.
    pub column: usize,
    /// Explicit confirmation, absent for read-only inspections.
    pub confirm: Option<ChoiceAction>,
}

impl App {
    pub(super) fn show_document(
        &mut self,
        title: &str,
        body: &str,
        confirm: Option<ChoiceAction>,
    ) {
        self.mode = Mode::Document(Box::new(DocumentView {
            title: title.to_owned(),
            lines: crate::content::clean(body)
                .lines()
                .map(str::to_owned)
                .collect(),
            row: 0,
            column: 0,
            confirm,
        }));
    }

    pub(super) fn document_key(&mut self, key: KeyEvent) -> Result<Action> {
        let Mode::Document(document) = &mut self.mode else {
            return Ok(Action::None);
        };
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') => self.mode = Mode::Normal,
            KeyCode::Down | KeyCode::Char('j') => {
                document.row = (document.row + 1)
                    .min(document.lines.len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                document.row = document.row.saturating_sub(1)
            }
            KeyCode::Right | KeyCode::Char('l' | 'L') => {
                document.column = (document.column + 8).min(4096)
            }
            KeyCode::Left | KeyCode::Char('h' | 'H') => {
                document.column = document.column.saturating_sub(8)
            }
            KeyCode::PageDown => {
                document.row = (document.row + 10)
                    .min(document.lines.len().saturating_sub(1))
            }
            KeyCode::PageUp => document.row = document.row.saturating_sub(10),
            KeyCode::Enter => {
                if let Some(action) = document.confirm.clone() {
                    let previous = std::mem::take(&mut self.mode);
                    if let Err(error) = self.apply_choice(action) {
                        self.mode = previous;
                        return Err(error);
                    }
                }
            }
            _ => {}
        }
        Ok(Action::None)
    }
}
