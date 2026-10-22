//! Formula preferences preserve source positions across asynchronous projections.
use super::{App, ReaderIdentity};
use crate::reader::{RenderOptions, to_display, to_source};
use anyhow::Result;

pub(super) struct FormulaPosition {
    identity: ReaderIdentity,
    cursor: Option<usize>,
    selection: Option<usize>,
    scroll: usize,
}

impl App {
    pub(super) fn set_formula_rendering(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        let enabled = match argument {
            "on" => true,
            "off" => false,
            "" | "toggle" => !self.workspace.latex_formulas,
            _ => anyhow::bail!("Use :latex-formulas on|off|toggle"),
        };
        self.workspace.latex_formulas = enabled;
        self.apply_formula_preferences();
        self.persist_preferences()?;
        self.status = format!(
            "LaTeX formulas {}",
            if enabled { "enabled" } else { "disabled" }
        );
        Ok(())
    }

    pub(super) fn formula_options(&self) -> RenderOptions {
        RenderOptions {
            latex_formulas: self.workspace.latex_formulas,
        }
    }

    pub(super) fn apply_formula_preferences(&mut self) {
        if !self
            .renderer
            .set_latex_formulas(self.workspace.latex_formulas)
        {
            return;
        }
        if self.formula_position.is_none()
            && let Some(identity) = self.reader_identity()
        {
            self.formula_position = Some(FormulaPosition {
                identity,
                cursor: self.formula_source(self.cursor_offset()),
                selection: self
                    .selection
                    .and_then(|offset| self.formula_source(offset)),
                scroll: self
                    .line_ranges
                    .get(self.scroll)
                    .map_or(0, |range| range.start),
            });
        }
        self.reader_tools.index =
            crate::reader::index::DocumentIndex::parse_with_options(
                &self.body,
                self.formula_options(),
            );
        self.wrap_reader();
        self.restore_formula_position();
    }

    fn formula_source(&self, offset: usize) -> Option<usize> {
        let end = crate::reader::cursor::next(&self.reader_text, offset);
        to_source(&self.reader_mappings, offset..end).map(|range| range.start)
    }

    pub(super) fn restore_formula_position(&mut self) -> bool {
        if self.renderer.preparing(&self.body, self.reader_width) {
            return false;
        }
        let Some(position) = self.formula_position.take() else {
            return false;
        };
        if self.reader_identity() != Some(position.identity) {
            return false;
        }
        self.pending_anchor = None;
        self.pending_reflow = false;
        let display = |source| {
            to_display(&self.reader_mappings, source..source.saturating_add(1))
                .first()
                .map(|range| range.start)
        };
        let cursor = position.cursor.and_then(display);
        self.selection = position.selection.and_then(display);
        self.scroll = self
            .line_ranges
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
