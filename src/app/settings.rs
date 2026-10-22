//! Live visual previews and normal-mode command bindings, separate from drafts.

mod editing;

use anyhow::{Result, ensure};
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
    pub(crate) const FIELD_COUNT: usize = 14;
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
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "config" | "config-reload" => {
                return self.configuration_command(verb);
            }
            "settings" => {
                self.mode = Mode::Settings(Box::new(SettingsDraft {
                    before: self.workspace.clone(),
                    field: 0,
                    editor: None,
                }))
            }
            "theme" => {
                ensure!(
                    ["lume", "nord", "gruvbox"].contains(&argument),
                    "Use :theme lume|nord|gruvbox"
                );
                self.workspace.theme = argument.into();
                self.persist_preferences()?;
            }
            "justify" => self.set_justification(argument)?,
            "latex-formulas" => self.set_formula_rendering(argument)?,
            "bind" => self.bind_command(argument)?,
            "unbind" => {
                self.workspace.keybindings.remove(argument);
                self.persist_preferences()?;
            }
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn set_justification(&mut self, argument: &str) -> Result<()> {
        let enabled = match argument {
            "on" => true,
            "off" => false,
            "" | "toggle" => !self.workspace.justify,
            _ => anyhow::bail!("Use :justify on|off|toggle"),
        };
        self.workspace.justify = enabled;
        self.cache_reader_rows();
        self.persist_preferences()?;
        self.status = format!(
            "Prose justification {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    fn bind_command(&mut self, argument: &str) -> Result<()> {
        let (key, command) = argument.split_once(' ').ok_or_else(|| {
            anyhow::anyhow!(
                "Use :bind KEY COMMAND, for example :bind ctrl-t toc"
            )
        })?;
        ensure!(
            self.workspace.keybindings.contains_key(key)
                || self.workspace.keybindings.len() < 128,
            "At most 128 custom key bindings"
        );
        validate_binding(key, command)?;
        self.workspace
            .keybindings
            .insert(key.into(), command.into());
        self.persist_preferences()?;
        self.status = format!(
            "Normal-mode {key} runs :{command}; editors keep their own keys"
        );
        Ok(())
    }

    pub(super) fn settings_key(&mut self, key: KeyEvent) -> Result<Action> {
        let justify = self.workspace.justify;
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('s')
        {
            self.confirm_setting_value()?;
            self.persist_preferences()?;
            self.mode = Mode::Normal;
            self.ensure_visible_focus();
            self.status = "Visual preferences saved".into();
        } else if matches!(&self.mode, Mode::Settings(draft) if draft.editor.is_some())
        {
            self.edit_setting_key(key)?;
        } else {
            self.preview_setting_key(key)?;
        }
        self.apply_formula_preferences();
        if self.workspace.justify != justify {
            self.cache_reader_rows();
        }
        Ok(Action::None)
    }

    fn preview_setting_key(&mut self, key: KeyEvent) -> Result<()> {
        let Mode::Settings(draft) = &mut self.mode else {
            return Ok(());
        };
        match key.code {
            KeyCode::Esc => {
                self.workspace = draft.before.clone();
                self.mode = Mode::Normal;
                self.status = "Visual preview discarded".into();
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
                change_visual(&mut self.workspace, draft.field, false)
            }
            KeyCode::Right | KeyCode::Char('l' | ' ') => {
                change_visual(&mut self.workspace, draft.field, true)
            }
            _ => {}
        }
        Ok(())
    }
}

fn change_visual(workspace: &mut Workspace, field: usize, forward: bool) {
    match field {
        0 => {
            let themes = ["lume", "nord", "gruvbox"];
            let current = themes
                .iter()
                .position(|theme| *theme == workspace.theme)
                .unwrap_or(0);
            let next = (current + if forward { 1 } else { 2 }) % themes.len();
            workspace.theme = themes[next].into();
        }
        1 => {
            workspace.reading_width =
                resize(workspace.reading_width, forward, 40, 140)
        }
        2 => {
            workspace.library_width =
                resize(workspace.library_width, forward, 16, 48)
        }
        3 => {
            workspace.article_width =
                resize(workspace.article_width, forward, 24, 80)
        }
        4 => workspace.compact = !workspace.compact,
        5 => workspace.show_library = !workspace.show_library,
        6 => workspace.show_articles = !workspace.show_articles,
        7 => {
            workspace.annotation_width =
                resize(workspace.annotation_width, forward, 24, 80)
        }
        8 => workspace.show_reader = !workspace.show_reader,
        9 => workspace.show_annotations = !workspace.show_annotations,
        10 => workspace.mouse = !workspace.mouse,
        11 => {
            workspace.fullscreen_width =
                resize(workspace.fullscreen_width, forward, 40, 140)
        }
        12 => workspace.justify = !workspace.justify,
        13 => workspace.latex_formulas = !workspace.latex_formulas,
        _ => {}
    }
}

fn resize(value: u16, forward: bool, minimum: u16, maximum: u16) -> u16 {
    if forward {
        value.saturating_add(1).min(maximum)
    } else {
        value.saturating_sub(1).max(minimum)
    }
}

fn valid_key_name(name: &str) -> bool {
    let base = name.strip_prefix("ctrl-").unwrap_or(name);
    let base = base.strip_prefix("alt-").unwrap_or(base);
    if matches!(
        base,
        "space"
            | "enter"
            | "tab"
            | "backtab"
            | "esc"
            | "up"
            | "down"
            | "left"
            | "right"
    ) {
        return true;
    }
    if let Some(number) = base
        .strip_prefix('f')
        .and_then(|number| number.parse::<u8>().ok())
    {
        return (1..=12).contains(&number);
    }
    base.chars().count() == 1 && !base.chars().any(char::is_control)
}

pub(crate) fn validate_binding(key: &str, command: &str) -> Result<()> {
    ensure!(
        valid_key_name(key),
        "Use a character, arrow, f1–f12 or named key with optional ctrl-/alt- prefixes"
    );
    ensure!(
        key != "ctrl-c" && !key.is_empty() && key.len() <= 32,
        "Ctrl-c is reserved for exit; key names use at most 32 bytes"
    );
    ensure!(
        command.len() <= 4096 && !command.chars().any(char::is_control),
        "Invalid bound command"
    );
    let verb = command.split_whitespace().next().unwrap_or("");
    ensure!(
        super::completion::resolve(verb).is_some(),
        "Bind an available : command"
    );
    Ok(())
}
