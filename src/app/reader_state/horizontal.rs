//! Per-block horizontal positions, independent of adjacent prose.
use super::ReaderState;
use std::collections::BTreeMap;

#[derive(Clone, Default)]
pub(in crate::app) struct Horizontal {
    active: Option<usize>,
    offsets: BTreeMap<usize, usize>,
}

impl ReaderState {
    pub(in crate::app) fn horizontal_for_row(&self, row: usize) -> usize {
        self.horizontal_view().for_row(row)
    }

    pub(super) fn horizontal_view(&self) -> HorizontalView<'_> {
        HorizontalView {
            groups: &self.projection.horizontal_groups,
            active: self.viewport.horizontal_state.active,
            offsets: &self.viewport.horizontal_state.offsets,
            current: self.viewport.horizontal,
        }
    }

    pub(super) fn select_horizontal_block(&mut self) {
        let group = self
            .projection
            .horizontal_groups
            .get(self.viewport.row)
            .copied()
            .flatten();
        if group == self.viewport.horizontal_state.active {
            return;
        }
        if let Some(previous) = self.viewport.horizontal_state.active {
            self.viewport
                .horizontal_state
                .offsets
                .insert(previous, self.viewport.horizontal);
        }
        self.viewport.horizontal = group
            .and_then(|key| {
                self.viewport.horizontal_state.offsets.get(&key).copied()
            })
            .unwrap_or(0);
        self.viewport.horizontal_state.active = group;
    }

    pub(in crate::app) fn scroll_horizontal_row(
        &mut self,
        row: usize,
        right: bool,
    ) {
        let Some(group) = self
            .projection
            .horizontal_groups
            .get(row)
            .copied()
            .flatten()
        else {
            return;
        };
        let current = self.horizontal_for_row(row);
        let offset = if right {
            current.saturating_add(1).min(4096)
        } else {
            current.saturating_sub(1)
        };
        if Some(group) == self.viewport.horizontal_state.active {
            self.viewport.horizontal = offset;
        } else {
            self.viewport.horizontal_state.offsets.insert(group, offset);
        }
        if current != offset {
            // Pointer scrolling can move a visible block without changing the
            // active cursor block or its cached horizontal viewport key.
            self.invalidate_auto_read_viewport();
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct HorizontalView<'a> {
    groups: &'a [Option<usize>],
    active: Option<usize>,
    offsets: &'a BTreeMap<usize, usize>,
    current: usize,
}

impl HorizontalView<'_> {
    pub(super) fn for_row(&self, row: usize) -> usize {
        let Some(group) = self.groups.get(row).copied().flatten() else {
            return 0;
        };
        if Some(group) == self.active {
            self.current
        } else {
            self.offsets.get(&group).copied().unwrap_or(0)
        }
    }
}
