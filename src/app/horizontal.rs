//! Each source block owns its viewport; movement never shifts its siblings.

use std::collections::BTreeMap;

use super::App;

#[derive(Clone, Default)]
pub(super) struct Horizontal {
    active: Option<usize>,
    offsets: BTreeMap<usize, usize>,
}

impl App {
    /// Horizontal viewport for this row's code block or table, in terminal cells.
    pub(crate) fn horizontal_for_row(&self, row: usize) -> usize {
        let group = self.horizontal_groups.get(row).copied().flatten();
        if group.is_none() {
            return 0;
        }
        if group == self.horizontal_state.active {
            return self.horizontal;
        }
        group
            .and_then(|key| self.horizontal_state.offsets.get(&key).copied())
            .unwrap_or(0)
    }

    pub(super) fn select_horizontal_block(&mut self) {
        let group = self
            .horizontal_groups
            .get(self.reader_cursor)
            .copied()
            .flatten();
        if group == self.horizontal_state.active {
            return;
        }
        if let Some(previous) = self.horizontal_state.active {
            self.horizontal_state
                .offsets
                .insert(previous, self.horizontal);
        }
        self.horizontal = group
            .and_then(|key| self.horizontal_state.offsets.get(&key).copied())
            .unwrap_or(0);
        self.horizontal_state.active = group;
    }

    pub(super) fn scroll_horizontal_row(&mut self, row: usize, right: bool) {
        let Some(group) = self.horizontal_groups.get(row).copied().flatten()
        else {
            return;
        };
        let current = self.horizontal_for_row(row);
        let offset = if right {
            current.saturating_add(1).min(4096)
        } else {
            current.saturating_sub(1)
        };
        if Some(group) == self.horizontal_state.active {
            self.horizontal = offset;
        } else {
            self.horizontal_state.offsets.insert(group, offset);
        }
    }
}
