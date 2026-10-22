//! Selected scalar preferences use the same bounded editor as other inputs.
use super::super::{App, Mode};
use crate::model::Workspace;
use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn begin_setting_edit(&mut self) {
        let Mode::Settings(draft) = &mut self.mode else {
            return;
        };
        if let Some(value) = value_text(&self.workspace, draft.field) {
            let mut editor = super::super::editor::textarea(&value);
            editor.select_all();
            draft.editor = Some(editor);
        } else {
            super::change_visual(&mut self.workspace, draft.field, true);
        }
    }

    pub(super) fn edit_setting_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Enter {
            return self.confirm_setting_value();
        }
        let Mode::Settings(draft) = &mut self.mode else {
            return Ok(());
        };
        if key.code == KeyCode::Esc {
            draft.editor = None;
        } else if let Some(editor) = &mut draft.editor {
            if key.modifiers.contains(KeyModifiers::CONTROL)
                && matches!(key.code, KeyCode::Char('j' | 'm'))
            {
                return Ok(());
            }
            if !matches!(key.code, KeyCode::Char(_))
                || editor.lines()[0].len() < 128
            {
                editor.input(key);
            }
        }
        Ok(())
    }

    pub(super) fn confirm_setting_value(&mut self) -> Result<()> {
        let Mode::Settings(draft) = &mut self.mode else {
            return Ok(());
        };
        let Some(editor) = &draft.editor else {
            return Ok(());
        };
        let value = editor.lines().join(" ");
        apply_value(&mut self.workspace, draft.field, value.trim())?;
        draft.editor = None;
        Ok(())
    }

    pub(in crate::app) fn paste_setting_value(&mut self, text: &str) -> bool {
        let Mode::Settings(draft) = &mut self.mode else {
            return false;
        };
        let Some(editor) = &mut draft.editor else {
            return false;
        };
        let mut remaining = 128_usize.saturating_sub(editor.lines()[0].len());
        let text: String = text
            .chars()
            .filter(|c| !c.is_control())
            .take_while(|c| {
                let fits = c.len_utf8() <= remaining;
                remaining = remaining.saturating_sub(c.len_utf8());
                fits
            })
            .collect();
        editor.insert_str(text);
        true
    }
}

fn value_text(workspace: &Workspace, field: usize) -> Option<String> {
    Some(match field {
        0 => workspace.theme.clone(),
        1 => workspace.reading_width.to_string(),
        2 => workspace.library_width.to_string(),
        3 => workspace.article_width.to_string(),
        7 => workspace.annotation_width.to_string(),
        11 => workspace.fullscreen_width.to_string(),
        _ => return None,
    })
}

fn apply_value(
    workspace: &mut Workspace,
    field: usize,
    text: &str,
) -> Result<()> {
    if field == 0 {
        ensure!(
            ["lume", "nord", "gruvbox"].contains(&text),
            "Use lume, nord or gruvbox"
        );
        workspace.theme = text.into();
        return Ok(());
    }
    let value: u16 = text
        .parse()
        .map_err(|_| anyhow::anyhow!("Enter a whole number"))?;
    let (destination, minimum, maximum) = match field {
        1 => (&mut workspace.reading_width, 40, 140),
        2 => (&mut workspace.library_width, 16, 48),
        3 => (&mut workspace.article_width, 24, 80),
        7 => (&mut workspace.annotation_width, 24, 80),
        11 => (&mut workspace.fullscreen_width, 40, 140),
        _ => anyhow::bail!("This setting has no editable scalar value"),
    };
    ensure!(
        (minimum..=maximum).contains(&value),
        "Use a value from {minimum} to {maximum}"
    );
    *destination = value;
    Ok(())
}
