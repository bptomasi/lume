//! Discoverable, English product guidance and stable section bookmarks.

use anyhow::{Result, ensure};

use super::{App, Mode};

/// The product guide shared by input scrolling and terminal presentation.
pub const HELP: &str = include_str!("help.txt");

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
        self.mode = Mode::Help;
        self.vim.clear_input();
        Ok(())
    }
}
