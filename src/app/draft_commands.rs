//! Ex writes share one vocabulary across the session and captured draft editor.
//!
//! Draft writes commit before closing. A failed write or session flush restores
//! the entire editor, including its captured identity and command line.

use crate::app::commands::routing;
use crate::command::{Command, resolve};
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
    let Some(spec) = resolve(word) else {
        return Ok(None);
    };
    let command = match spec.command {
        Command::Write => ExCommand::Write(Destination::Editor),
        Command::Wq => ExCommand::Write(Destination::Reader),
        Command::Wqa => ExCommand::Write(Destination::Quit),
        Command::Quit => ExCommand::Close {
            destination: if spec.application_aliases.contains(&word) {
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
        command: routing::SessionCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::SessionCommand as C;

        ensure!(
            argument.is_empty(),
            "Session commands do not accept arguments"
        );
        let quit = match command {
            C::SaveStatus => {
                self.show_save_status();
                return Ok(Action::None);
            }
            C::Write => false,
            C::Wq | C::Wqa | C::Quit => true,
        };
        self.write_session(quit)
    }

    fn write_session(&mut self, quit: bool) -> Result<Action> {
        self.flush_session()?;
        self.save_session()?;
        self.flush_session()?;
        self.status = "Session saved".into();
        if quit {
            return Ok(Action::Quit);
        }
        Ok(Action::None)
    }

    fn show_save_status(&mut self) {
        while let Some(error) = self
            .session_writer
            .as_ref()
            .and_then(|writer| writer.poll_error())
        {
            self.session_problem = Some(error);
        }
        let state = if self.session_failed() {
            "Failed writes remain outstanding"
        } else if self.session_pending() {
            "Writes are pending"
        } else {
            "No outstanding session save failures"
        };
        let error = self
            .session_problem
            .as_deref()
            .unwrap_or("None recorded in this session");
        let text = format!(
            "# Session saving\n\n{state}\n\nLast error:\n{error}\n\n\
            This writer saves reading positions and workspace/layout history.\n\
            Notes use a separate durable writer.\n\n\
            :w retries outstanding session saves and waits for confirmation.\n\
            Successful writes to unrelated items do not clear a failed save."
        );
        self.show_document("Save diagnostics · Esc closes", &text, None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::command_suggestions;

    #[test]
    fn registry_write_and_quit_aliases_preserve_editor_destinations()
    -> Result<()> {
        for spec in command_suggestions("") {
            for spelling in
                std::iter::once(spec.name).chain(spec.aliases.iter().copied())
            {
                assert_editor_destination(spec, spelling)?;
            }
        }
        for alias in ["w", "w!", "wq", "x", "q!", "qa", "wqa!"] {
            assert!(parse(&format!("{alias} argument")).is_err());
        }
        Ok(())
    }

    fn assert_editor_destination(
        spec: &crate::command::CommandSpec,
        spelling: &str,
    ) -> Result<()> {
        let parsed = parse(spelling)?;
        match spec.command {
            Command::Write => assert!(matches!(
                parsed,
                Some(ExCommand::Write(Destination::Editor))
            )),
            Command::Wq => assert!(matches!(
                parsed,
                Some(ExCommand::Write(Destination::Reader))
            )),
            Command::Wqa => assert!(matches!(
                parsed,
                Some(ExCommand::Write(Destination::Quit))
            )),
            Command::Quit => {
                let Some(ExCommand::Close {
                    destination,
                    discard,
                }) = parsed
                else {
                    anyhow::bail!("Quit alias did not parse: {spelling}");
                };
                assert_eq!(discard, spelling.ends_with('!'));
                assert_eq!(
                    matches!(destination, Destination::Quit),
                    spec.application_aliases.contains(&spelling)
                );
            }
            _ => assert!(
                parsed.is_none(),
                "Unexpected session alias: {spelling}"
            ),
        }
        Ok(())
    }
}
