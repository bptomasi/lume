//! Sidebar matching stays in memory; article queries reuse the paged worker.
//!
//! Cached normalized names and matching identities change only with the query
//! or catalog. Drawing and pointer lookup share those same identities.
//! Matching indices retain catalog order for binary position lookups.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, InputKind, Mode};

impl App {
    pub(super) fn move_library_match(
        &self,
        forward: bool,
        distance: usize,
    ) -> Option<usize> {
        self.nav.move_library_match(forward, distance)
    }

    pub(super) fn clear_library_search(&mut self) {
        self.nav.clear_library_search()
    }

    pub(super) fn cancel_library_search(&mut self) {
        self.nav.cancel_library_search()
    }

    pub(super) fn update_library_search(&mut self, query: &str) {
        self.nav.update_library_search(query)
    }

    pub(super) fn navigation_position(&self, index: usize) -> Option<usize> {
        self.nav.navigation_position(index)
    }

    /// Current sidebar name filter, independent of article and reader search.
    pub fn library_query(&self) -> &str {
        self.nav.library_query()
    }

    pub(super) fn begin_library_search(&mut self) {
        let selected = self
            .nav
            .navigation()
            .get(self.nav.nav_selected())
            .map_or_else(|| self.nav.view().clone(), |item| item.view.clone());
        self.nav.begin_library_search(selected);
        self.begin_input(
            InputKind::LibrarySearch,
            self.nav.library_query().to_owned(),
        );
    }

    pub(super) fn commit_library_search(&mut self) -> Result<()> {
        self.navigation_position(self.nav.nav_selected())
            .context("No matching library destinations")?;
        let selected = self.nav.nav_selected();
        self.set_view(self.nav.navigation()[selected].view.clone())?;
        self.nav.commit_library_search();
        if matches!(self.nav.view(), super::View::Group(_)) {
            self.toggle_navigation_group("")?;
            return Ok(());
        }
        self.focus_pane(Focus::Articles)
    }

    pub(super) fn pane_search_key(&mut self, key: KeyEvent) -> Result<bool> {
        let Mode::Input(input) = self.interaction.mode() else {
            return Ok(false);
        };
        let library = matches!(input.kind, InputKind::LibrarySearch);
        if !library && !matches!(input.kind, InputKind::Search) {
            return Ok(false);
        }
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        let forward = match key.code {
            KeyCode::Down => true,
            KeyCode::Up => false,
            KeyCode::Char('j' | 'n') if control => true,
            KeyCode::Char('k' | 'p') if control => false,
            _ => return Ok(false),
        };
        if library {
            self.pin_navigation_viewport();
            if let Some(selected) = self.move_library_match(forward, 1) {
                let destination = self.nav.navigation()[selected].view.clone();
                self.select_library_destination(&destination)?;
            }
        } else {
            self.move_selection(forward, 1)?;
        }
        Ok(true)
    }
}
