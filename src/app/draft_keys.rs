//! A small modal layer delegates text edits and Unicode cursor work to TextArea.
//!
//! Inserted or pasted colons remain literal. Only the explicit normal-mode
//! command line can execute Ex commands; it never becomes part of note text.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::CursorMove;

use super::{Action, App, Input, Mode, editor::multiline};

/// Editing layer within a captured annotation or study-card buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EditMode {
    /// Keys insert text using the native text editor.
    Insert,
    /// Vim-inspired motions and commands, leaving the draft intact.
    Normal,
    /// Await the second key of ZZ (write/close) or ZQ (discard/close).
    PendingZ,
    /// Separate, bounded Ex command line; Enter executes and Esc cancels it.
    Command(String),
    /// A regular expression scoped to the current note buffer.
    Search(String),
}

impl App {
    pub(super) fn draft_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<Action>> {
        let Mode::Input(input) = &mut self.mode else {
            return Ok(None);
        };
        if !multiline(&input.kind) {
            return Ok(None);
        }
        if key.code == KeyCode::Esc {
            input.edit_mode = EditMode::Normal;
            return Ok(Some(Action::None));
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('s' | 'e'))
        {
            return Ok(None);
        }
        if let EditMode::Search(query) = &mut input.edit_mode {
            if let Some(query) = command_key(query, key) {
                input.editor.set_search_pattern(query)?;
                input.editor.search_forward(true);
                input.edit_mode = EditMode::Normal;
            }
            return Ok(Some(Action::None));
        }
        let command = match &mut input.edit_mode {
            EditMode::Insert => return Ok(None),
            EditMode::Command(line) => command_key(line, key),
            _ => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(None);
                }
                normal_key(input, key.code)
            }
        };
        command.map_or(Ok(Some(Action::None)), |text| {
            self.draft_command(&text).map(Some)
        })
    }
}

fn command_key(line: &mut String, key: KeyEvent) -> Option<String> {
    match key.code {
        KeyCode::Enter => return Some(line.clone()),
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            line.clear();
        }
        KeyCode::Backspace => {
            line.pop();
        }
        KeyCode::Char(character)
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !character.is_control()
                && line.len() + character.len_utf8() <= 256 =>
        {
            line.push(character);
        }
        _ => {}
    }
    None
}

fn normal_key(input: &mut Input, code: KeyCode) -> Option<String> {
    if input.edit_mode == EditMode::PendingZ {
        input.edit_mode = EditMode::Normal;
        match code {
            KeyCode::Char('Z') => return Some("x".into()),
            KeyCode::Char('Q') => return Some("q!".into()),
            _ => {}
        }
    }
    if let Some(movement) = motion(code) {
        input.editor.move_cursor(movement);
        return None;
    }
    match code {
        KeyCode::Char(':') => {
            input.edit_mode = EditMode::Command(String::new())
        }
        KeyCode::Char('/') => input.edit_mode = EditMode::Search(String::new()),
        KeyCode::Char('n') => {
            input.editor.search_forward(false);
        }
        KeyCode::Char('N') => {
            input.editor.search_back(false);
        }
        KeyCode::Char('Z') => input.edit_mode = EditMode::PendingZ,
        KeyCode::Char('i') => input.edit_mode = EditMode::Insert,
        KeyCode::Char('a' | 'A' | 'I' | 'o' | 'O') => insert_at(input, code),
        KeyCode::Char('x') => {
            input.editor.delete_next_char();
        }
        KeyCode::Char('u') => {
            input.editor.undo();
        }
        _ => {}
    }
    input.text = input.editor.lines().join("\n");
    None
}

fn motion(code: KeyCode) -> Option<CursorMove> {
    Some(match code {
        KeyCode::Char('h') | KeyCode::Left => CursorMove::Back,
        KeyCode::Char('l') | KeyCode::Right => CursorMove::Forward,
        KeyCode::Char('j') | KeyCode::Down => CursorMove::Down,
        KeyCode::Char('k') | KeyCode::Up => CursorMove::Up,
        KeyCode::Char('w') => CursorMove::WordForward,
        KeyCode::Char('b') => CursorMove::WordBack,
        KeyCode::Char('e') => CursorMove::WordEnd,
        KeyCode::Char('0') | KeyCode::Home => CursorMove::Head,
        KeyCode::Char('$') | KeyCode::End => CursorMove::End,
        KeyCode::Char('G') => CursorMove::Bottom,
        _ => return None,
    })
}

fn insert_at(input: &mut Input, code: KeyCode) {
    input.editor.move_cursor(match code {
        KeyCode::Char('a') => CursorMove::Forward,
        KeyCode::Char('I' | 'O') => CursorMove::Head,
        _ => CursorMove::End,
    });
    if matches!(code, KeyCode::Char('o' | 'O')) && input.text.len() < 16_384 {
        input.editor.insert_newline();
        if code == KeyCode::Char('O') {
            input.editor.move_cursor(CursorMove::Up);
        }
    }
    input.edit_mode = EditMode::Insert;
}
