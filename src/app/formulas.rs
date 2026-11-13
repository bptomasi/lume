//! Formula preferences preserve source positions across asynchronous projections.
use super::App;
use crate::reader::RenderOptions;
use anyhow::Result;

impl App {
    pub(super) fn set_formula_rendering(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        let enabled = match argument {
            "on" => true,
            "graphics" | "text" => {
                self.layout.set_formula_graphics(argument == "graphics");
                true
            }
            "off" => false,
            "" | "toggle" => !self.layout.workspace().latex_formulas,
            _ => {
                anyhow::bail!("Use :latex-formulas on|off|toggle|text|graphics")
            }
        };
        self.layout.set_formulas(enabled);
        self.apply_formula_preferences();
        self.persist_preferences()?;
        self.interaction.report(format!(
            "LaTeX formulas {}",
            if !enabled {
                "disabled"
            } else if self.layout.workspace().latex_graphics
                && self.uses_sixel_images()
            {
                "enabled · graphics"
            } else if self.layout.workspace().latex_graphics {
                "enabled · text fallback; graphics need Sixel"
            } else {
                "enabled · text"
            }
        ));
        Ok(())
    }

    pub(super) fn formula_options(&self) -> RenderOptions {
        RenderOptions {
            latex_formulas: self.layout.workspace().latex_formulas,
            graphical_formulas: self.layout.workspace().latex_graphics
                && self.uses_sixel_images(),
        }
    }

    pub(super) fn apply_formula_preferences(&mut self) {
        self.reader.apply_formula_preferences(self.render_context());
    }

    pub(super) fn pending_formula_motion(
        &self,
    ) -> Option<super::reader_state::FormulaMotion> {
        self.reader.pending_formula_motion(
            self.layout.focus() == super::Focus::Reader
                && matches!(self.interaction.mode(), super::Mode::Normal),
        )
    }

    pub(super) fn retarget_formula_motion(
        &mut self,
        before: Option<super::reader_state::FormulaMotion>,
    ) {
        let active = self.layout.focus() == super::Focus::Reader
            && matches!(self.interaction.mode(), super::Mode::Normal);
        self.reader.retarget_formula_motion(before, active);
    }
}
