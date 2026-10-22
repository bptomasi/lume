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

#[derive(PartialEq, Eq)]
pub(super) struct FormulaMotion {
    row: usize,
    column: usize,
    scroll: usize,
    selection: Option<usize>,
}

impl App {
    pub(super) fn set_formula_rendering(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        let enabled = match argument {
            "on" => true,
            "graphics" | "text" => {
                self.workspace.latex_graphics = argument == "graphics";
                true
            }
            "off" => false,
            "" | "toggle" => !self.workspace.latex_formulas,
            _ => {
                anyhow::bail!("Use :latex-formulas on|off|toggle|text|graphics")
            }
        };
        self.workspace.latex_formulas = enabled;
        self.apply_formula_preferences();
        self.persist_preferences()?;
        self.status = format!(
            "LaTeX formulas {}",
            if !enabled {
                "disabled"
            } else if self.workspace.latex_graphics && self.uses_sixel_images()
            {
                "enabled · graphics"
            } else if self.workspace.latex_graphics {
                "enabled · text fallback; graphics need Sixel"
            } else {
                "enabled · text"
            }
        );
        Ok(())
    }

    pub(super) fn formula_options(&self) -> RenderOptions {
        RenderOptions {
            latex_formulas: self.workspace.latex_formulas,
            graphical_formulas: self.workspace.latex_graphics
                && self.uses_sixel_images(),
        }
    }

    pub(super) fn apply_formula_preferences(&mut self) {
        if !self.renderer.set_render_options(self.formula_options()) {
            return;
        }
        if self.formula_position.is_none() {
            self.formula_position = self.capture_formula_position();
        }
        self.reader_tools.index =
            crate::reader::index::DocumentIndex::parse_with_options(
                &self.body,
                self.formula_options(),
            );
        self.wrap_reader();
        self.restore_formula_position();
    }

    fn capture_formula_position(&self) -> Option<FormulaPosition> {
        Some(FormulaPosition {
            identity: self.reader_identity()?,
            cursor: self.formula_source(self.cursor_offset()),
            selection: self
                .selection
                .and_then(|offset| self.formula_source(offset)),
            scroll: self
                .line_ranges
                .get(self.scroll)
                .map_or(0, |range| range.start),
        })
    }

    pub(super) fn pending_formula_motion(&self) -> Option<FormulaMotion> {
        (self.formula_position.is_some()
            && self.focus == super::Focus::Reader
            && matches!(self.mode, super::Mode::Normal))
        .then_some(FormulaMotion {
            row: self.reader_cursor,
            column: self.reader_column,
            scroll: self.scroll,
            selection: self.selection,
        })
    }

    pub(super) fn retarget_formula_motion(
        &mut self,
        before: Option<FormulaMotion>,
    ) {
        if self.formula_position.is_some()
            && before.is_some()
            && before != self.pending_formula_motion()
        {
            // A later user movement owns the source anchor when layout arrives.
            self.formula_position = self.capture_formula_position();
        }
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
