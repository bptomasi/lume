//! A bounded secret entry never becomes command text or an annotation draft.

use super::{Action, App, Mode};
use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// Ephemeral, redacted server credential entry; only the final store exposes it.
#[derive(Clone)]
pub struct ServerToken {
    address: String,
    value: String,
}

impl std::fmt::Debug for ServerToken {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output
            .debug_struct("ServerToken")
            .field("address", &self.address)
            .field("value", &"[redacted]")
            .finish()
    }
}

impl ServerToken {
    /// Canonical destination shown before committing the private credential.
    pub fn address(&self) -> &str {
        &self.address
    }

    /// Visible masking only; drawing never exposes credential bytes.
    pub fn masked(&self, width: u16) -> String {
        "•".repeat(self.value.chars().count().min(usize::from(width)))
    }

    pub(in crate::app) fn paste(&mut self, text: &str) -> Result<()> {
        let text = text.trim_end_matches(['\r', '\n']);
        ensure!(
            self.value.len().saturating_add(text.len()) <= 4096,
            "API token exceeds 4,096 bytes"
        );
        ensure!(
            !text.chars().any(char::is_control),
            "API token cannot contain control characters"
        );
        self.value.push_str(text);
        Ok(())
    }
}

impl App {
    pub(super) fn begin_server_token(
        &mut self,
        address: &str,
    ) -> Result<Action> {
        let address = crate::source::server_url(address)?;
        self.mode = Mode::ServerToken(ServerToken {
            address,
            value: String::new(),
        });
        Ok(Action::None)
    }

    pub(in crate::app) fn server_token_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Action> {
        if key.code == KeyCode::Esc {
            self.mode = Mode::Normal;
            return Ok(Action::None);
        }
        if key.code == KeyCode::Enter {
            return self.save_server_token();
        }
        let Mode::ServerToken(input) = &mut self.mode else {
            return Ok(Action::None);
        };
        match key.code {
            KeyCode::Backspace => {
                input.value.pop();
            }
            KeyCode::Char('u')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                input.value.clear()
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                input.paste(&character.to_string())?;
            }
            _ => {}
        }
        Ok(Action::None)
    }

    fn save_server_token(&mut self) -> Result<Action> {
        let Mode::ServerToken(input) = &self.mode else {
            return Ok(Action::None);
        };
        let token = crate::model::ApiToken::from(input.value.as_str());
        let id = self.store.add_server_token(&input.address, &token)?;
        self.mode = Mode::Normal;
        self.finish_server_connection(id)
    }
}
