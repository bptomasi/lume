//! Selected scalar preferences use the same bounded editor as other inputs.
use super::super::App;
use crate::model::Workspace;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

impl App {
    pub(super) fn begin_setting_edit(&mut self) {
        let Some(draft) = self.interaction.settings_mut() else {
            return;
        };
        if let Some(value) = value_text(self.layout.workspace(), draft.field) {
            let mut editor = super::super::editor::textarea(&value);
            editor.select_all();
            draft.editor = Some(editor);
        } else {
            self.layout.preview_visual(draft.field, true);
        }
    }

    pub(super) fn edit_setting_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Enter {
            return self.confirm_setting_value();
        }
        let Some(draft) = self.interaction.settings_mut() else {
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
        let Some(draft) = self.interaction.settings_mut() else {
            return Ok(());
        };
        let Some(editor) = &draft.editor else {
            return Ok(());
        };
        let value = editor.lines().join(" ");
        self.layout.preview_value(draft.field, value.trim())?;
        draft.editor = None;
        Ok(())
    }

    pub(in crate::app) fn paste_setting_value(&mut self, text: &str) -> bool {
        let Some(draft) = self.interaction.settings_mut() else {
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
        16 => workspace.auto_read.min_active_seconds.to_string(),
        17 => workspace.auto_read.min_coverage_percent.to_string(),
        _ => return None,
    })
}
