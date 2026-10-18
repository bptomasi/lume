//! Cursor-aware editor operations with bounded drafts and native undo history.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::{CursorMove, TextArea};

use super::{Action, App, Input, InputKind, Mode};

pub(super) fn multiline(kind: &InputKind) -> bool {
    matches!(
        kind,
        InputKind::Comment { .. }
            | InputKind::Highlight { .. }
            | InputKind::StudyCard { .. }
    )
}

pub(super) fn textarea(text: &str) -> TextArea<'static> {
    let mut editor = TextArea::from(text.split('\n'));
    editor.move_cursor(CursorMove::Bottom);
    editor.move_cursor(CursorMove::End);
    editor.set_max_histories(128);
    use ratatui::style::{Color, Style};
    editor.set_style(
        Style::new()
            .fg(Color::Rgb(224, 219, 208))
            .bg(Color::Rgb(23, 23, 26)),
    );
    editor.set_cursor_style(
        Style::new()
            .fg(Color::Rgb(18, 18, 20))
            .bg(Color::Rgb(225, 179, 109)),
    );
    editor.set_cursor_line_style(Style::default());
    editor.set_selection_style(Style::new().bg(Color::Rgb(37, 35, 33)));
    editor
}

impl App {
    /// Return text only for an annotation/study draft eligible for external editing.
    pub fn external_draft(&self) -> Option<&str> {
        let Mode::Input(input) = &self.mode else {
            return None;
        };
        multiline(&input.kind).then_some(input.text.as_str())
    }

    /// Insert paste as text; pasted controls never execute normal-mode actions.
    ///
    /// # Errors
    /// Returns storage errors while updating a live search.
    pub fn paste(&mut self, text: &str) -> Result<()> {
        if let Mode::Picker(picker) = &mut self.mode {
            let remaining = 4096_usize.saturating_sub(picker.query.len());
            picker.query.extend(
                text.chars().filter(|c| !c.is_control()).take(remaining),
            );
            return self.update_picker();
        }
        let Mode::Input(input) = &mut self.mode else {
            self.status = "Open an editor before pasting".into();
            return Ok(());
        };
        let remaining = 16_384_usize.saturating_sub(input.text.len());
        let text = text
            .chars()
            .filter(|c| !c.is_control() || *c == '\n' && multiline(&input.kind))
            .collect::<String>();
        let end = text.floor_char_boundary(text.len().min(remaining));
        input.editor.insert_str(&text[..end]);
        input.text = input.editor.lines().join("\n");
        input.completion = 0;
        self.update_search()
    }

    pub(crate) fn begin_input(&mut self, kind: InputKind, text: String) {
        self.pending_g = false;
        self.vim.clear_input();
        if matches!(kind, InputKind::Search) {
            self.retained_article = None;
        }
        let editor = textarea(&text);
        self.mode = Mode::Input(Box::new(Input {
            kind,
            text,
            previous_search: self.search.clone(),
            editor,
            completion: 0,
        }));
    }

    pub(super) fn input_key(&mut self, key: KeyEvent) -> Result<Action> {
        if key.code == KeyCode::Esc {
            return self.cancel_input();
        }
        if self.completion_key(key) {
            return Ok(Action::None);
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let Mode::Input(input) = &mut self.mode else {
            return Ok(Action::None);
        };
        if control && key.code == KeyCode::Char('e') && multiline(&input.kind) {
            return Ok(Action::ExternalEditor);
        }
        if control && key.code == KeyCode::Char('s')
            || key.code == KeyCode::Enter && !multiline(&input.kind)
        {
            return self.submit_input();
        }
        if input.text.len() + inserted_bytes(key.code) > 16_384 && !control {
            return Ok(Action::None);
        }
        if control && key.code == KeyCode::Char('u') {
            input.editor.select_all();
            input.editor.cut();
        } else if control && key.code == KeyCode::Char('z') {
            input.editor.undo();
        } else if control && key.code == KeyCode::Char('r') {
            input.editor.redo();
        } else {
            input.editor.input(key);
        }
        let bytes = input.editor.lines().iter().map(String::len).sum::<usize>()
            + input.editor.lines().len().saturating_sub(1);
        if bytes > 16_384 {
            input.editor.undo();
        }
        input.text = input.editor.lines().join("\n");
        input.completion = 0;
        self.update_search()?;
        Ok(Action::None)
    }

    fn update_search(&mut self) -> Result<()> {
        if let Mode::Input(input) = &self.mode
            && matches!(input.kind, InputKind::Search)
        {
            self.search.clone_from(&input.text);
            self.page_offset = 0;
            self.selected = 0;
            self.filter()?;
        }
        Ok(())
    }

    fn cancel_input(&mut self) -> Result<Action> {
        if let Mode::Input(input) = &self.mode
            && matches!(input.kind, InputKind::Search)
        {
            self.search.clone_from(&input.previous_search);
            self.page_offset = 0;
            self.filter()?;
        }
        self.mode = Mode::Normal;
        Ok(Action::None)
    }

    pub(super) fn submit_input(&mut self) -> Result<Action> {
        if !self.prepare_command() {
            return Ok(Action::None);
        }
        let Mode::Input(input) = std::mem::take(&mut self.mode) else {
            return Ok(Action::None);
        };
        match self.apply_input(&input) {
            Ok(action) => Ok(action),
            Err(error) => {
                self.mode = Mode::Input(input);
                Err(error)
            }
        }
    }
}

fn inserted_bytes(code: KeyCode) -> usize {
    match code {
        KeyCode::Char(character) => character.len_utf8(),
        KeyCode::Enter => 1,
        _ => 0,
    }
}

impl App {
    /// Replace an open annotation draft after a successful external edit.
    ///
    /// # Errors
    /// Rejects oversized drafts and attempts without an open annotation editor.
    pub fn replace_draft(&mut self, text: String) -> Result<()> {
        anyhow::ensure!(text.len() <= 16_384, "Draft exceeds 16 KiB");
        let Mode::Input(input) = &mut self.mode else {
            anyhow::bail!("No draft is open");
        };
        anyhow::ensure!(
            multiline(&input.kind),
            "Open an annotation editor first"
        );
        input.editor = textarea(&text);
        input.text = text;
        Ok(())
    }
}
