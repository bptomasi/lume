//! Sidebar matching stays in memory; article queries reuse the paged worker.
//!
//! Cached normalized names and matching identities change only with the query
//! or catalog. Drawing and pointer lookup share those same identities.
//! Matching indices retain catalog order for binary position lookups.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::{App, Focus, InputKind, Mode};

#[derive(Default)]
pub(super) struct LibrarySearch {
    query: String,
    names: Vec<String>,
    matches: Vec<usize>,
    before: Option<(String, super::View)>,
}

impl App {
    /// Current sidebar name filter, independent of article and reader searches.
    pub fn library_query(&self) -> &str {
        &self.library_search.query
    }

    pub(crate) fn navigation_indices(
        &self,
    ) -> impl DoubleEndedIterator<Item = usize> + Clone + '_ {
        let count = if self.library_search.query.is_empty() {
            self.navigation.len()
        } else {
            0
        };
        (0..count).chain(self.library_search.matches.iter().copied())
    }

    pub(crate) fn navigation_position(&self, index: usize) -> Option<usize> {
        if self.library_search.query.is_empty() {
            return self.navigation.get(index).map(|_| index);
        }
        self.library_search.matches.binary_search(&index).ok()
    }

    pub(super) fn refresh_library_search(&mut self) {
        self.library_search.names = self
            .navigation
            .iter()
            .map(|item| format!("{} {}", item.group, item.title).to_lowercase())
            .collect();
        self.filter_library_search(false);
    }

    fn filter_library_search(&mut self, narrowing: bool) {
        if self.library_search.query.is_empty() {
            self.library_search.matches.clear();
            return;
        }
        let query = self.library_search.query.to_lowercase();
        if narrowing {
            self.library_search.matches.retain(|index| {
                self.library_search.names[*index].contains(&query)
            });
        } else {
            self.library_search.matches.clear();
            self.library_search.matches.extend(
                self.library_search.names.iter().enumerate().filter_map(
                    |(index, name)| name.contains(&query).then_some(index),
                ),
            );
        }
        if self
            .library_search
            .matches
            .binary_search(&self.nav_selected)
            .is_err()
            && let Some(first) = self.library_search.matches.first()
        {
            self.nav_selected = *first;
        }
    }

    pub(super) fn begin_library_search(&mut self) {
        let selected = self
            .navigation
            .get(self.nav_selected)
            .map_or_else(|| self.view.clone(), |item| item.view.clone());
        self.library_search.before =
            Some((self.library_search.query.clone(), selected));
        self.begin_input(
            InputKind::LibrarySearch,
            self.library_search.query.clone(),
        );
    }

    pub(super) fn update_library_search(&mut self, query: &str) {
        if self.library_search.query == query {
            return;
        }
        let previous = self.library_search.query.to_lowercase();
        let narrowing =
            !previous.is_empty() && query.to_lowercase().starts_with(&previous);
        self.navigation_first = 0;
        self.navigation_skip = 0;
        self.library_search.query = query.to_owned();
        self.filter_library_search(narrowing);
    }

    pub(super) fn cancel_library_search(&mut self) {
        if let Some((query, selected)) = self.library_search.before.take() {
            self.update_library_search(&query);
            if let Some(index) = self
                .navigation
                .iter()
                .position(|item| item.view.same_destination(&selected))
            {
                self.nav_selected = index;
            }
        }
    }

    pub(super) fn clear_library_search(&mut self) {
        self.library_search.before = None;
        self.update_library_search("");
    }

    pub(super) fn commit_library_search(&mut self) -> Result<()> {
        self.navigation_position(self.nav_selected)
            .context("No matching library destinations")?;
        let selected = self.nav_selected;
        self.set_view(self.navigation[selected].view.clone())?;
        self.library_search.before = None;
        self.focus_pane(Focus::Articles)
    }

    pub(super) fn move_library_match(
        &self,
        forward: bool,
        distance: usize,
    ) -> Option<usize> {
        if self.library_search.query.is_empty() {
            let next = if forward {
                self.nav_selected
                    .saturating_add(distance)
                    .min(self.navigation.len().saturating_sub(1))
            } else {
                self.nav_selected.saturating_sub(distance)
            };
            return self.navigation.get(next).map(|_| next);
        }
        let matches = &self.library_search.matches;
        let position = matches.binary_search(&self.nav_selected).ok()?;
        let next = if forward {
            position
                .saturating_add(distance)
                .min(matches.len().saturating_sub(1))
        } else {
            position.saturating_sub(distance)
        };
        let selected = *matches.get(next)?;
        Some(selected)
    }

    pub(super) fn pane_search_key(&mut self, key: KeyEvent) -> Result<bool> {
        let Mode::Input(input) = &self.mode else {
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
                self.nav_selected = selected;
            }
        } else {
            self.move_selection(forward, 1)?;
        }
        Ok(true)
    }
}
