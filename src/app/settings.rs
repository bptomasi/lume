//! Live visual previews and normal-mode command bindings, separate from drafts.

use crate::app::commands::routing;

mod editing;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::model::Workspace;

use super::{Action, App, Mode};

/// Visual preferences are provisional until Ctrl-s; Esc restores the snapshot.
#[derive(Clone, Debug)]
pub struct SettingsDraft {
    /// Complete preferences observed before opening this modal.
    pub before: Workspace,
    /// Visual preference row; additions retain the original seven field indices.
    pub field: usize,
    /// Editor for the selected theme or numeric value; Enter applies its preview.
    pub(crate) editor: Option<tui_textarea::TextArea<'static>>,
}

impl SettingsDraft {
    pub(crate) const FIELD_COUNT: usize = 18;
}

/// Stable key spelling accepted by `:bind`; no draft/editor key is remapped.
pub fn key_name(key: KeyEvent) -> String {
    let code = match key.code {
        KeyCode::Char(' ') => "space".into(),
        KeyCode::Char(character) => character.to_string(),
        KeyCode::F(number) => format!("f{number}"),
        KeyCode::Enter => "enter".into(),
        KeyCode::Tab => "tab".into(),
        KeyCode::BackTab => "backtab".into(),
        KeyCode::Esc => "esc".into(),
        KeyCode::Up => "up".into(),
        KeyCode::Down => "down".into(),
        KeyCode::Left => "left".into(),
        KeyCode::Right => "right".into(),
        _ => return String::new(),
    };
    let mut prefix = String::new();
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        prefix.push_str("ctrl-");
    }
    if key.modifiers.contains(KeyModifiers::ALT) {
        prefix.push_str("alt-");
    }
    format!("{prefix}{code}")
}

impl App {
    pub(super) fn settings_command(
        &mut self,
        command: routing::SettingsCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::SettingsCommand as C;

        match command {
            C::Settings => self.interaction.enter(Mode::Settings(Box::new(
                SettingsDraft {
                    before: self.layout.workspace().clone(),
                    field: 0,
                    editor: None,
                },
            ))),
            C::Theme => {
                self.layout.set_theme(argument)?;
                self.persist_preferences()?;
            }
            C::Justify => self.set_justification(argument)?,
            C::Bind => self.bind_command(argument)?,
            C::Unbind => {
                self.layout.unbind(argument);
                self.persist_preferences()?;
            }
        }
        Ok(Action::None)
    }

    fn set_justification(&mut self, argument: &str) -> Result<()> {
        let enabled = match argument {
            "on" => true,
            "off" => false,
            "" | "toggle" => !self.layout.workspace().justify,
            _ => anyhow::bail!("Use :justify on|off|toggle"),
        };
        self.layout.set_justification(enabled);
        self.apply_reader_justification();
        self.persist_preferences()?;
        self.interaction.report(format!(
            "Prose justification {}",
            if enabled { "enabled" } else { "disabled" }
        ));
        Ok(())
    }

    fn bind_command(&mut self, argument: &str) -> Result<()> {
        let (key, command) = argument.split_once(' ').ok_or_else(|| {
            anyhow::anyhow!(
                "Use :bind KEY COMMAND, for example :bind ctrl-t toc"
            )
        })?;
        self.layout.bind(key, command)?;
        self.persist_preferences()?;
        self.interaction.report(format!(
            "Normal-mode {key} runs :{command}; editors keep their own keys"
        ));
        Ok(())
    }

    pub(super) fn settings_key(&mut self, key: KeyEvent) -> Result<Action> {
        let justify = self.layout.workspace().justify;
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('s')
        {
            self.confirm_setting_value()?;
            self.persist_preferences()?;
            self.interaction.enter(Mode::Normal);
            self.ensure_visible_focus();
            self.interaction.report("Visual preferences saved".into());
        } else if matches!(self.interaction.mode(), Mode::Settings(draft) if draft.editor.is_some())
        {
            self.edit_setting_key(key)?;
        } else {
            self.preview_setting_key(key)?;
        }
        self.apply_formula_preferences();
        if self.layout.workspace().justify != justify {
            self.apply_reader_justification();
        }
        Ok(Action::None)
    }

    fn preview_setting_key(&mut self, key: KeyEvent) -> Result<()> {
        let Some(draft) = self.interaction.settings_mut() else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => {
                self.layout.restore_preview(draft.before.clone());
                self.interaction.enter(Mode::Normal);
                self.interaction.report("Visual preview discarded".into());
            }
            KeyCode::Enter => self.begin_setting_edit(),
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Tab => {
                draft.field = (draft.field + 1) % SettingsDraft::FIELD_COUNT;
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::BackTab => {
                draft.field = (draft.field + SettingsDraft::FIELD_COUNT - 1)
                    % SettingsDraft::FIELD_COUNT;
            }
            KeyCode::Left | KeyCode::Char('h') => {
                self.layout.preview_visual(draft.field, false)
            }
            KeyCode::Right | KeyCode::Char('l' | ' ') => {
                self.layout.preview_visual(draft.field, true)
            }
            _ => {}
        }
        Ok(())
    }
}
