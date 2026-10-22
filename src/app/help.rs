//! Discoverable, English product guidance and stable section bookmarks.

use std::sync::OnceLock;

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent};
use tui_textarea::TextArea;

use super::{App, Mode};

/// The product guide shared by input scrolling and terminal presentation.
pub const HELP: &str = include_str!("help.txt");

#[derive(Default)]
pub(crate) struct HelpSearch {
    pub(crate) query: String,
    pub(crate) editor: Option<TextArea<'static>>,
    pub(crate) hits: Vec<usize>,
    pub(crate) selected: usize,
    before: Option<(String, usize, usize)>,
}

impl HelpSearch {
    fn update(&mut self, query: String) {
        self.query = query;
        let needle = self.query.trim().to_lowercase();
        self.hits.clear();
        if !needle.is_empty() {
            static LINES: OnceLock<Vec<String>> = OnceLock::new();
            let lines = LINES
                .get_or_init(|| HELP.lines().map(str::to_lowercase).collect());
            self.hits.extend(lines.iter().enumerate().filter_map(
                |(row, text)| {
                    needle
                        .split_whitespace()
                        .all(|word| text.contains(word))
                        .then_some(row)
                },
            ));
        }
        self.selected = 0;
    }
}

impl App {
    pub(super) fn show_help(&mut self, topic: &str) -> Result<()> {
        let topic = topic.to_ascii_uppercase();
        let section = if topic.is_empty() { "LAYOUT" } else { &topic };
        let position = HELP.lines().position(|line| {
            line.starts_with("  ──") && line.contains(section)
        });
        ensure!(
            position.is_some(),
            "Help topics: layout, mouse, vim, selection, lists, sources, study, notes, notebook, recovery, recommendations, commands"
        );
        self.help_scroll = position.unwrap_or(0);
        self.help_search = HelpSearch::default();
        self.mode = Mode::Help;
        self.vim.clear_input();
        Ok(())
    }

    pub(super) fn help_search_key(&mut self, key: KeyEvent) -> bool {
        if self.help_search.editor.is_some() {
            self.edit_help_search(key);
            return true;
        }
        match key.code {
            KeyCode::Char('/') => {
                self.help_search.before = Some((
                    self.help_search.query.clone(),
                    self.help_scroll,
                    self.help_search.selected,
                ));
                self.help_search.editor =
                    Some(super::editor::textarea(&self.help_search.query));
            }
            KeyCode::Char('n' | 'N') if !self.help_search.hits.is_empty() => {
                let length = self.help_search.hits.len();
                let step = if key.code == KeyCode::Char('N') {
                    length - 1
                } else {
                    1
                };
                self.help_search.selected =
                    (self.help_search.selected + step) % length;
                self.help_scroll =
                    self.help_search.hits[self.help_search.selected];
            }
            KeyCode::Esc if !self.help_search.query.is_empty() => {
                self.help_search = HelpSearch::default();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn paste_help_search(&mut self, text: &str) {
        let Some(editor) = &mut self.help_search.editor else {
            return;
        };
        let mut available = 4096_usize.saturating_sub(editor.lines()[0].len());
        let text: String = text
            .chars()
            .filter(|c| !c.is_control())
            .take_while(|c| {
                let fits = c.len_utf8() <= available;
                available = available.saturating_sub(c.len_utf8());
                fits
            })
            .collect();
        editor.insert_str(text);
        self.refresh_help_search();
    }

    fn refresh_help_search(&mut self) {
        let Some(editor) = &self.help_search.editor else {
            return;
        };
        let query = editor.lines().join(" ");
        self.help_search.update(query);
        if let Some(first) = self.help_search.hits.first() {
            self.help_scroll = *first;
        }
    }

    fn edit_help_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if let Some((query, scroll, selected)) =
                    self.help_search.before.take()
                {
                    self.help_search.update(query);
                    self.help_scroll = scroll;
                    self.help_search.selected = selected;
                }
                self.help_search.editor = None;
            }
            KeyCode::Enter => {
                self.help_search.editor = None;
                self.help_search.before = None;
            }
            _ => {
                if let Some(editor) = &mut self.help_search.editor {
                    if matches!(key.code, KeyCode::Char(_))
                        && editor.lines()[0].len() >= 4096
                    {
                        return;
                    }
                    editor.input(key);
                    self.refresh_help_search();
                }
            }
        }
    }
}
