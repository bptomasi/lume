//! Active publisher coverage is separate from application write admission.
mod state;

use super::ReaderState;
use crate::app::ReaderIdentity;
use std::time::Instant;

#[derive(Default)]
pub(super) struct AutoRead {
    state: state::State,
    unfocused: bool,
}

impl ReaderState {
    pub(in crate::app) fn observe_reading(
        &mut self,
        now: Instant,
        policy: crate::model::AutoReadSettings,
        active: bool,
    ) -> Option<ReaderIdentity> {
        let Some(identity) = self.reader_identity() else {
            self.auto_read.state.pause();
            return None;
        };
        self.auto_read.state.source(
            identity,
            self.projection.article_end,
            policy,
        );
        if !active || self.auto_read.unfocused {
            self.auto_read.state.pause();
            return None;
        }
        self.observe_auto_read_viewport();
        self.auto_read.state.tick(now).then_some(identity)
    }

    pub(in crate::app) fn auto_read_admitted(&mut self) {
        self.auto_read.state.committed_intent();
    }

    pub(super) fn observe_auto_read_viewport(&mut self) {
        let view = (
            self.viewport.first,
            self.viewport.height,
            self.viewport.width,
            self.viewport.horizontal,
        );
        if self.auto_read.state.view == Some(view) {
            return;
        }
        self.auto_read.state.view = Some(view);
        let last = self
            .viewport
            .first
            .saturating_add(self.viewport.height)
            .min(self.projection.ranges.len());
        for row in self.viewport.first..last {
            let Some(visible) = self.visible_publisher_range(row) else {
                break;
            };
            self.auto_read.state.record(visible);
        }
    }

    pub(in crate::app) fn auto_read_window_focus(&mut self, focused: bool) {
        self.auto_read.unfocused = !focused;
        self.auto_read.state.pause();
    }

    pub(in crate::app) fn auto_read_manual_state(&mut self, article: i64) {
        self.auto_read.state.block(article);
    }

    pub(super) fn auto_read_text_changed(&mut self) {
        self.auto_read.state.reset_text();
    }

    pub(super) fn invalidate_auto_read_viewport(&mut self) {
        self.auto_read.state.view = None;
    }
}

impl ReaderState {
    fn visible_publisher_range(
        &self,
        row: usize,
    ) -> Option<std::ops::Range<usize>> {
        let range = self.projection.ranges.get(row)?;
        if range.start >= self.projection.article_end {
            return None;
        }
        let next = self
            .projection
            .ranges
            .get(row + 1)
            .map_or(self.projection.article_end, |next| next.start)
            .min(self.projection.article_end);
        let visible = range.start..next;
        Some(if self.projection.code.get(row) == Some(&true) {
            self.clip_code_coverage(row, visible, range.len())
        } else {
            visible
        })
    }

    fn clip_code_coverage(
        &self,
        row: usize,
        mut visible: std::ops::Range<usize>,
        row_bytes: usize,
    ) -> std::ops::Range<usize> {
        let skip = self.horizontal_for_row(row);
        let end_column = self.row_column(row, row_bytes);
        if skip > 0 && skip >= end_column {
            return visible.end..visible.end;
        }
        let start = visible.start;
        let mut left = self.row_byte_at(row, skip);
        if self.row_column(row, left) < skip {
            // A partly clipped tab or wide grapheme paints blank cells.
            let text =
                self.projection.lines.get(row).map_or("", String::as_str);
            left = crate::reader::cursor::next(text, left);
        }
        visible.start += left;
        let right = skip.saturating_add(self.viewport.width);
        // Cursor positioning clamps to the last grapheme. Coverage must
        // include that grapheme and the separator when the row end fits.
        if right < end_column {
            visible.end = start + self.row_byte_at(row, right);
        }
        visible.end = visible.end.max(visible.start);
        visible
    }
}
