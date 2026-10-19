//! Ex writes share one vocabulary across the session and captured draft editor.
//!
//! Draft writes commit before closing. A failed write or session flush restores
//! the entire editor, including its captured identity and command line.

use anyhow::{Result, bail, ensure};

use super::{Action, App, Input, Mode};

#[derive(Clone, Copy)]
pub(super) enum Destination {
    Editor,
    Reader,
    Quit,
}

#[derive(Clone, Copy)]
pub(super) enum ExCommand {
    Write(Destination),
    Close {
        destination: Destination,
        discard: bool,
    },
}

pub(super) fn parse(text: &str) -> Result<Option<ExCommand>> {
    let mut words = text.split_whitespace();
    let word = words.next().unwrap_or("");
    let verb = word.strip_suffix('!').unwrap_or(word);
    let command = match verb {
        "w" | "write" | "wa" | "wall" => ExCommand::Write(Destination::Editor),
        "wq" | "x" | "xit" | "exit" => ExCommand::Write(Destination::Reader),
        "wqa" | "wqall" | "xa" | "xall" => ExCommand::Write(Destination::Quit),
        "q" | "quit" | "qa" | "qall" => ExCommand::Close {
            destination: if matches!(verb, "qa" | "qall") {
                Destination::Quit
            } else {
                Destination::Reader
            },
            discard: word.ends_with('!'),
        },
        _ => return Ok(None),
    };
    ensure!(words.next().is_none(), ":{word} does not accept arguments");
    Ok(Some(command))
}

impl App {
    pub(super) fn draft_command(&mut self, text: &str) -> Result<Action> {
        if self.note_editor_command(text)? {
            return Ok(Action::None);
        }
        let Some(command) = parse(text)? else {
            bail!("Unknown editor command · use :w, :wq, :wqa or :q!");
        };
        if self.defer_draft_command(command)? {
            return Ok(Action::None);
        }
        let Mode::Input(mut input) = std::mem::take(&mut self.mode) else {
            bail!("Open a note editor first");
        };
        match self.apply_draft_command(&mut input, command) {
            Ok((action, Destination::Editor)) => {
                input.edit_mode = super::EditMode::Normal;
                self.mode = Mode::Input(input);
                Ok(action)
            }
            Ok((action, _)) => Ok(action),
            Err(error) => {
                self.mode = Mode::Input(input);
                Err(error)
            }
        }
    }

    fn apply_draft_command(
        &mut self,
        input: &mut Input,
        command: ExCommand,
    ) -> Result<(Action, Destination)> {
        let destination = match command {
            ExCommand::Write(destination) => {
                self.write_draft(input)?;
                destination
            }
            ExCommand::Close {
                destination,
                discard,
            } => {
                ensure!(
                    discard || !input.modified(),
                    "Unsaved changes · :w saves · :q! discards"
                );
                self.discard_draft_checkpoint(input)?;
                destination
            }
        };
        let action = if matches!(destination, Destination::Quit) {
            self.write_session(true)?
        } else {
            Action::None
        };
        Ok((action, destination))
    }

    pub(super) fn session_command(
        &mut self,
        text: &str,
    ) -> Result<Option<Action>> {
        let Some(command) = parse(text)? else {
            return Ok(None);
        };
        let quit = !matches!(command, ExCommand::Write(Destination::Editor));
        self.write_session(quit).map(Some)
    }

    fn write_session(&mut self, quit: bool) -> Result<Action> {
        self.save_session()?;
        self.flush_session()?;
        self.status = "Session saved".into();
        if quit {
            return Ok(Action::Quit);
        }
        Ok(Action::None)
    }
}
