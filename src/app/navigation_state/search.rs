//! Search responsibilities owned by navigation.

use super::{NavigationState, View};

impl NavigationState {
    pub fn library_query(&self) -> &str {
        &self.library_search.query
    }

    pub(in crate::app) fn navigation_position(
        &self,
        index: usize,
    ) -> Option<usize> {
        if self.library_search.query.is_empty() {
            return self.navigation.get(index).map(|_| index);
        }
        self.library_search.matches.binary_search(&index).ok()
    }

    pub(in crate::app) fn refresh_library_search(&mut self) {
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

    pub(in crate::app) fn update_library_search(&mut self, query: &str) {
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

    pub(in crate::app) fn cancel_library_search(&mut self) {
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

    pub(in crate::app) fn clear_library_search(&mut self) {
        self.library_search.before = None;
        self.update_library_search("");
    }

    pub(in crate::app) fn move_library_match(
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

    pub(in crate::app) fn begin_library_search(&mut self, selected: View) {
        self.library_search.before =
            Some((self.library_search.query.clone(), selected));
    }

    pub(in crate::app) fn commit_library_search(&mut self) {
        self.library_search.before = None;
    }
}
