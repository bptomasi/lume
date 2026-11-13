//! Discoverable ex commands with literal prefix matching and bounded work.
//!
//! Suggestions describe the existing dispatch contracts; aliases resolve to
//! their canonical verb before dispatch. Tab only edits the draft. Enter on
//! an incomplete verb fills it first, so browsing never executes an action.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, InputKind};

pub(super) use crate::command::resolve;
pub use crate::command::{CommandSpec, command_suggestions};

impl App {
    pub(super) fn completion_key(&mut self, key: KeyEvent) -> bool {
        let Some(input) = self.interaction.draft_mut() else {
            return false;
        };
        if matches!(input.kind, InputKind::NoteSearch)
            && key.code == KeyCode::Tab
        {
            if let Some(text) = crate::search::notes::complete(&input.text) {
                input.text = text;
                input.editor = super::editor::textarea(&input.text);
            }
            return true;
        }
        if matches!(input.kind, InputKind::Search) && key.code == KeyCode::Tab {
            return complete_search(input);
        }
        if !matches!(input.kind, InputKind::Command) {
            return false;
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let forward = match key.code {
            KeyCode::Tab => {
                complete(input);
                return true;
            }
            KeyCode::Down => true,
            KeyCode::Up | KeyCode::BackTab => false,
            KeyCode::Char('n') if control => true,
            KeyCode::Char('p') if control => false,
            _ => return false,
        };
        let count = command_suggestions(&input.text).len();
        if count > 0 {
            input.completion = if forward {
                (input.completion + 1) % count
            } else {
                (input.completion + count - 1) % count
            };
        }
        true
    }

    pub(super) fn prepare_command(&mut self) -> bool {
        let Some(input) = self.interaction.draft_mut() else {
            return true;
        };
        if !matches!(input.kind, InputKind::Command) {
            return true;
        }
        let verb = input.text.split_whitespace().next().unwrap_or("");
        let Some(command) = resolve(verb) else {
            return !complete(input);
        };
        let argument = input.text.trim_start().get(verb.len()..).unwrap_or("");
        let required = command
            .usage
            .split_whitespace()
            .nth(1)
            .is_some_and(|argument| !argument.starts_with('['));
        if required && argument.trim().is_empty() {
            self.interaction
                .report(format!("Usage: :{}", command.usage));
            return false;
        }
        true
    }
}

fn complete_search(input: &mut super::Input) -> bool {
    let Some(completed) = crate::search::complete(&input.text) else {
        return false;
    };
    input.text = completed;
    input.editor = super::editor::textarea(&input.text);
    true
}

fn complete(input: &mut super::Input) -> bool {
    let choices = command_suggestions(&input.text);
    let Some(command) = choices.get(input.completion) else {
        return false;
    };
    let verb = input.text.split_whitespace().next().unwrap_or("");
    let argument = input.text.trim_start().get(verb.len()..).unwrap_or("");
    let argument = if argument.is_empty() && command.usage != command.name {
        " "
    } else {
        argument
    };
    let completed = format!("{}{argument}", command.name);
    if completed.len() > 16_384 {
        return false;
    }
    input.text = completed;
    input.editor = super::editor::textarea(&input.text);
    input.completion = 0;
    true
}
