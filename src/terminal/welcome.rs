//! Entry input is consumed here so it cannot navigate or mutate the library.

use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};

use super::InputOutcome;

pub(super) fn handle(visible: &mut bool, event: Event) -> InputOutcome {
    match event {
        Event::Key(key) if key.kind == KeyEventKind::Press => {
            if key.code == KeyCode::Char('c')
                && key.modifiers.contains(KeyModifiers::CONTROL)
            {
                return InputOutcome::Quit;
            }
            // Modifier keys and terminal capability replies are not entry keys.
            if matches!(key.code, KeyCode::Modifier(_) | KeyCode::Null) {
                return InputOutcome::Idle;
            }
            *visible = false;
            InputOutcome::Redraw
        }
        Event::Resize(_, _) => InputOutcome::Redraw,
        _ => InputOutcome::Idle,
    }
}

#[cfg(test)]
mod tests;
