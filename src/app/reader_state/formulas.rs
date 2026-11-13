//! Source positions survive asynchronous formula projections and user motions.
use super::{ReaderState, RenderContext};
use crate::app::ReaderIdentity;
use crate::reader::{to_display, to_source};

pub(super) struct FormulaPosition {
    identity: ReaderIdentity,
    cursor: Option<usize>,
    selection: Option<usize>,
    scroll: usize,
}

#[derive(PartialEq, Eq)]
pub(in crate::app) struct FormulaMotion {
    row: usize,
    column: usize,
    scroll: usize,
    selection: Option<usize>,
}

impl ReaderState {
    pub(in crate::app) fn apply_formula_preferences(
        &mut self,
        context: RenderContext,
    ) {
        if !self.renderer.set_render_options(context.options) {
            return;
        }
        if self.viewport.formula_position.is_none() {
            self.viewport.formula_position = self.capture_formula_position();
        }
        self.projection.tools.index =
            crate::reader::index::DocumentIndex::parse_with_options(
                &self.publisher.body,
                context.options,
            );
        self.wrap_reader(context);
        self.restore_formula_position();
    }

    fn capture_formula_position(&self) -> Option<FormulaPosition> {
        Some(FormulaPosition {
            identity: self.reader_identity()?,
            cursor: self.formula_source(self.cursor_offset()),
            selection: self
                .viewport
                .selection
                .and_then(|offset| self.formula_source(offset)),
            scroll: self
                .projection
                .source_ranges
                .get(self.viewport.first)
                .map_or(0, |range| range.start),
        })
    }

    pub(in crate::app) fn pending_formula_motion(
        &self,
        active: bool,
    ) -> Option<FormulaMotion> {
        (self.viewport.formula_position.is_some() && active).then_some(
            FormulaMotion {
                row: self.viewport.row,
                column: self.viewport.byte,
                scroll: self.viewport.first,
                selection: self.viewport.selection,
            },
        )
    }

    pub(in crate::app) fn retarget_formula_motion(
        &mut self,
        before: Option<FormulaMotion>,
        active: bool,
    ) {
        if self.viewport.formula_position.is_some()
            && before.is_some()
            && before != self.pending_formula_motion(active)
        {
            // A later user movement owns the source anchor when layout arrives.
            self.viewport.formula_position = self.capture_formula_position();
        }
    }

    pub(super) fn formula_source(&self, offset: usize) -> Option<usize> {
        let end = crate::reader::cursor::next(&self.projection.text, offset);
        to_source(&self.projection.mappings, offset..end)
            .map(|range| range.start)
    }

    pub(super) fn restore_formula_position(&mut self) -> bool {
        if self
            .renderer
            .preparing(&self.publisher.body, self.viewport.width)
        {
            return false;
        }
        let Some(position) = self.viewport.formula_position.take() else {
            return false;
        };
        if self.reader_identity() != Some(position.identity) {
            return false;
        }
        self.cancel_restore();
        self.viewport.pending_reflow = false;
        let display = |source| {
            to_display(
                &self.projection.mappings,
                source..source.saturating_add(1),
            )
            .first()
            .map(|range| range.start)
        };
        let cursor = position.cursor.and_then(display);
        self.viewport.selection = position.selection.and_then(display);
        self.viewport.first = self
            .projection
            .source_ranges
            .iter()
            .position(|range| range.end > position.scroll)
            .unwrap_or(0);
        if let Some(cursor) = cursor {
            self.place_cursor(cursor);
        }
        self.ensure_cursor_visible();
        self.clamp_scroll();
        true
    }
}
