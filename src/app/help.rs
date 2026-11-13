//! Discoverable, English product guidance and stable section bookmarks.

use std::sync::OnceLock;

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent};
use tui_textarea::TextArea;

use super::{App, Mode};

/// The product guide shared by input scrolling and terminal presentation.
pub const HELP: &str = include_str!("help.txt");

#[derive(Default)]
struct HelpSearch {
    query: String,
    editor: Option<TextArea<'static>>,
    hits: Vec<usize>,
    selected: usize,
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

#[derive(Default)]
pub(super) struct HelpState {
    scroll: usize,
    search: HelpSearch,
}

/// Borrowed guide search and scroll presentation.
pub struct HelpView<'a> {
    /// Guide line at the top of the viewport.
    pub scroll: usize,
    /// Current search text.
    pub query: &'a str,
    /// Search editor when a live query is active.
    pub editor: Option<&'a TextArea<'static>>,
    /// Matching guide rows, reused across frames.
    pub hits: &'a [usize],
    /// Selected matching row.
    pub selected: usize,
}

impl HelpState {
    pub(super) fn view(&self) -> HelpView<'_> {
        HelpView {
            scroll: self.scroll,
            query: &self.search.query,
            editor: self.search.editor.as_ref(),
            hits: &self.search.hits,
            selected: self.search.selected,
        }
    }

    pub(super) fn scroll(&mut self, forward: bool, amount: usize) {
        self.scroll = if forward {
            self.scroll
                .saturating_add(amount)
                .min(HELP.lines().count().saturating_sub(1))
        } else {
            self.scroll.saturating_sub(amount)
        };
    }

    pub(super) fn seek(&mut self, end: bool) {
        self.scroll = if end {
            HELP.lines().count().saturating_sub(1)
        } else {
            0
        };
    }

    pub(super) fn open(&mut self, topic: &str) -> Result<()> {
        let topic = topic.to_ascii_uppercase();
        let section = if topic.is_empty() { "LAYOUT" } else { &topic };
        let position = HELP.lines().position(|line| {
            line.starts_with("  ──") && line.contains(section)
        });
        ensure!(
            position.is_some(),
            "Help topics: layout, mouse, vim, selection, lists, sources, study, notes, notebook, recovery, recommendations, commands"
        );
        self.scroll = position.unwrap_or(0);
        self.search = HelpSearch::default();
        Ok(())
    }

    pub(super) fn search_key(&mut self, key: KeyEvent) -> bool {
        if self.search.editor.is_some() {
            self.edit_help_search(key);
            return true;
        }
        match key.code {
            KeyCode::Char('/') => {
                self.search.before = Some((
                    self.search.query.clone(),
                    self.scroll,
                    self.search.selected,
                ));
                self.search.editor =
                    Some(super::editor::textarea(&self.search.query));
            }
            KeyCode::Char('n' | 'N') if !self.search.hits.is_empty() => {
                let length = self.search.hits.len();
                let step = if key.code == KeyCode::Char('N') {
                    length - 1
                } else {
                    1
                };
                self.search.selected = (self.search.selected + step) % length;
                self.scroll = self.search.hits[self.search.selected];
            }
            KeyCode::Esc if !self.search.query.is_empty() => {
                self.search = HelpSearch::default();
            }
            _ => return false,
        }
        true
    }

    pub(super) fn paste_search(&mut self, text: &str) {
        let Some(editor) = &mut self.search.editor else {
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
        let Some(editor) = &self.search.editor else {
            return;
        };
        let query = editor.lines().join(" ");
        self.search.update(query);
        if let Some(first) = self.search.hits.first() {
            self.scroll = *first;
        }
    }

    fn edit_help_search(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                if let Some((query, scroll, selected)) =
                    self.search.before.take()
                {
                    self.search.update(query);
                    self.scroll = scroll;
                    self.search.selected = selected;
                }
                self.search.editor = None;
            }
            KeyCode::Enter => {
                self.search.editor = None;
                self.search.before = None;
            }
            _ => {
                if let Some(editor) = &mut self.search.editor {
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

impl App {
    pub(super) fn show_help(&mut self, topic: &str) -> Result<()> {
        self.interaction.help_input().open(topic)?;
        self.interaction.enter(Mode::Help);
        self.interaction.vim_input().clear_input();
        Ok(())
    }

    /// Borrow the guide presentation without copying its search matches.
    pub fn help(&self) -> HelpView<'_> {
        self.interaction.help().view()
    }
}
