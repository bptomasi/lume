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
        input.paste_text(text);
        self.observe_draft();
        self.update_search()
    }

    pub(crate) fn begin_input(&mut self, kind: InputKind, text: String) {
        self.pending_g = false;
        self.vim.clear_input();
        if matches!(kind, InputKind::Search) {
            self.retained_article = None;
        }
        self.mode =
            Mode::Input(Box::new(Input::new(kind, text, self.search.clone())));
        if let Mode::Input(input) = &mut self.mode
            && matches!(input.kind, InputKind::NoteSearch)
        {
            input.previous_search.clone_from(&input.text);
        }
    }

    pub(super) fn input_key(&mut self, key: KeyEvent) -> Result<Action> {
        if let Some(action) = self.draft_key(key)? {
            return Ok(action);
        }
        if key.code == KeyCode::Esc {
            return self.cancel_input();
        }
        if self.pane_search_key(key)? {
            return Ok(Action::None);
        }
        if self.completion_key(key) {
            self.update_search()?;
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
            if multiline(&input.kind) && self.personal_writes.is_some() {
                self.request_draft_write(
                    super::draft_commands::Destination::Reader,
                )?;
                return Ok(Action::None);
            }
            return self.submit_input();
        }
        input.edit_key(key);
        self.complete_typed_reference(key)?;
        self.update_search()?;
        Ok(Action::None)
    }

    fn complete_typed_reference(&mut self, key: KeyEvent) -> Result<()> {
        let Mode::Input(input) = &self.mode else {
            return Ok(());
        };
        let (row, column) = input.editor.cursor();
        let prefix = input.editor.lines()[row]
            .chars()
            .take(column)
            .collect::<String>();
        if matches!(
            input.kind,
            InputKind::Comment { .. } | InputKind::Highlight { .. }
        ) && prefix.ends_with("[[")
            && key.code == KeyCode::Char('[')
        {
            self.note_link_completion("")?;
        }
        Ok(())
    }

    fn update_search(&mut self) -> Result<()> {
        if let Mode::Input(input) = &self.mode
            && matches!(input.kind, InputKind::LibrarySearch)
        {
            let query = input.text.clone();
            self.update_library_search(&query);
            return Ok(());
        }
        if let Mode::Input(input) = &self.mode
            && matches!(input.kind, InputKind::NoteSearch)
        {
            let query = input.text.clone();
            if let Err(error) = self.set_notebook_query(&query) {
                self.status = error.to_string();
            }
            return Ok(());
        }
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
        if let Mode::Input(input) = &self.mode {
            if matches!(input.kind, InputKind::LibrarySearch) {
                self.cancel_library_search();
            } else if matches!(input.kind, InputKind::ReaderSearch) {
                self.reader_tools.needle.clear();
                self.reader_tools.hits.clear();
            }
        }
        if let Mode::Input(input) = &self.mode
            && matches!(input.kind, InputKind::NoteSearch)
        {
            let query = input.previous_search.clone();
            self.set_notebook_query(&query)?;
        }
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
        let Mode::Input(mut input) = std::mem::take(&mut self.mode) else {
            return Ok(Action::None);
        };
        match self.apply_input(&mut input) {
            Ok(action) => Ok(action),
            Err(error) => {
                self.mode = Mode::Input(input);
                Err(error)
            }
        }
    }
}

impl Input {
    pub(super) fn draft_target(&self) -> Option<crate::model::DraftTarget> {
        use crate::model::DraftTarget;
        Some(match &self.kind {
            InputKind::Comment { article, comment } => DraftTarget::Note {
                article: *article,
                note: *comment,
            },
            InputKind::Highlight { article, passage } => DraftTarget::Passage {
                article: *article,
                passage: passage.clone(),
            },
            InputKind::StudyCard {
                article,
                comment,
                card,
            } => DraftTarget::Card {
                article: *article,
                note: *comment,
                card: *card,
            },
            _ => return None,
        })
    }

    fn paste_text(&mut self, text: &str) {
        if let super::EditMode::Command(line) = &mut self.edit_mode {
            let text =
                text.chars().filter(|c| !c.is_control()).collect::<String>();
            let remaining = 256_usize.saturating_sub(line.len());
            let end = text.floor_char_boundary(remaining.min(text.len()));
            line.push_str(&text[..end]);
            return;
        }
        self.edit_mode = super::EditMode::Insert;
        let remaining = 16_384_usize.saturating_sub(self.text.len());
        let text = text
            .chars()
            .filter(|c| {
                !c.is_control()
                    || matches!(c, '\n' | '\t') && multiline(&self.kind)
            })
            .collect::<String>();
        let end = text.floor_char_boundary(text.len().min(remaining));
        self.editor.insert_str(&text[..end]);
        self.text = self.editor.lines().join("\n");
        self.completion = 0;
    }

    fn edit_key(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if self.text.len() + inserted_bytes(key.code) > 16_384 && !control {
            return;
        }
        if control && key.code == KeyCode::Char('u') {
            self.editor.select_all();
            self.editor.cut();
        } else if control && key.code == KeyCode::Char('z') {
            self.editor.undo();
        } else if control && key.code == KeyCode::Char('r') {
            self.editor.redo();
        } else {
            self.editor.input(key);
        }
        let bytes = self.editor.lines().iter().map(String::len).sum::<usize>()
            + self.editor.lines().len().saturating_sub(1);
        if bytes > 16_384 {
            self.editor.undo();
        }
        self.text = self.editor.lines().join("\n");
        self.completion = 0;
    }

    pub(crate) fn new(
        kind: InputKind,
        text: String,
        previous_search: String,
    ) -> Self {
        let editor = textarea(&text);
        Self {
            kind,
            saved_text: text.clone(),
            initial_kind: None,
            text,
            previous_search,
            editor,
            completion: 0,
            edit_mode: super::EditMode::Insert,
        }
    }

    /// Whether closing this buffer would discard text or a captured passage.
    pub fn modified(&self) -> bool {
        self.text != self.saved_text
            || matches!(self.kind, InputKind::Highlight { .. })
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
        self.observe_draft();
        Ok(())
    }
}
