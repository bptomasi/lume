//! Bounded LaTeX conversion to selectable terminal text, outside drawing.
//!
//! math-core owns TeX semantics; MathML is projected to Unicode rather than
//! claiming pixel typesetting. Unsupported or excessive input stays verbatim.

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay, MaxExpansions};
mod embellishments;
mod fractions;
pub(super) mod graphics;
mod projection;
mod scripts;

#[derive(Default)]
pub(super) struct Math {
    converter: Option<LatexToMathML>,
    count: usize,
    bytes: usize,
    cache: std::collections::VecDeque<(String, bool, Option<String>)>,
    graphic_cache: std::collections::VecDeque<(
        String,
        bool,
        Option<graphics::FormulaImage>,
    )>,
}

impl Math {
    pub(super) fn render(
        &mut self,
        latex: &str,
        block: bool,
    ) -> Option<String> {
        self.count += 1;
        self.bytes += latex.len();
        if self.count > 256 || self.bytes > 65536 || !admissible(latex) {
            return None;
        }
        if let Some((_, _, text)) = self
            .cache
            .iter()
            .find(|(source, display, _)| source == latex && *display == block)
        {
            return text.clone();
        }
        let text = self.convert(latex, block);
        if self.cache.len() == 32 {
            self.cache.pop_front();
        }
        self.cache.push_back((latex.into(), block, text.clone()));
        text
    }

    fn convert(&mut self, latex: &str, block: bool) -> Option<String> {
        if self.converter.is_none() {
            let config = MathCoreConfig {
                max_expansions: MaxExpansions(64),
                // These restrictions concern browser MathML layout, not our
                // explicit Unicode projection of wide and lower accents.
                allow_unreliable_rendering: true,
                ..MathCoreConfig::default()
            };
            self.converter = LatexToMathML::new(config, Vec::new()).ok();
        }
        let display = if block {
            MathDisplay::Block
        } else {
            MathDisplay::Inline
        };
        let math = self
            .converter
            .as_ref()?
            .convert_with_local_state(latex, display)
            .ok()?
            .mathml;
        if math.len() > 65536 {
            return None;
        }
        // math-core emits these two HTML-only MathML entity names.
        // Normalize its generated output without permitting XML DTDs.
        let math = math
            .replace("&ApplyFunction;", "&#x2061;")
            .replace("&InvisibleTimes;", "&#x2062;");
        let document = roxmltree::Document::parse_with_options(
            &math,
            roxmltree::ParsingOptions {
                nodes_limit: 4096,
                ..Default::default()
            },
        )
        .ok()?;
        let text = projection::project(document.root_element(), 0)?;
        (text.len() <= 8192).then(|| text.trim().to_owned())
    }

    pub(super) fn graphic(
        &mut self,
        latex: &str,
        block: bool,
    ) -> Option<graphics::FormulaImage> {
        if self.count > 256 || self.bytes > 65536 {
            return None;
        }
        if let Some((_, _, image)) = self
            .graphic_cache
            .iter()
            .find(|(source, display, _)| source == latex && *display == block)
        {
            return image.clone();
        }
        let image = graphics::render(latex, block);
        if self.graphic_cache.len() == 32 {
            self.graphic_cache.pop_front();
        }
        self.graphic_cache
            .push_back((latex.into(), block, image.clone()));
        image
    }
}

/// Small inline expressions stay in the paragraph, including variables and
/// command names. Reserve pixels for layouts that benefit from typesetting.
pub(super) fn needs_graphics(latex: &str, text: &str, block: bool) -> bool {
    block
        || text.contains(['_', '^'])
        || super::cursor::column(text, text.len()) > 16
        || [
            r"\frac", r"\dfrac", r"\tfrac", r"\sqrt", r"\sum", r"\prod",
            r"\int", r"\begin", r"\binom",
        ]
        .iter()
        .any(|command| latex.contains(command))
}

fn admissible(latex: &str) -> bool {
    if latex.len() > 4096 || latex.chars().filter(|c| *c == '\\').count() > 256
    {
        return false;
    }
    // Paired currency amounts can resemble math delimiters in prose.
    if latex.starts_with(|c: char| c.is_ascii_digit())
        && latex.contains(char::is_whitespace)
        && !latex.contains(['\\', '^', '_', '=', '+', '/', '*', '{'])
    {
        return false;
    }
    let mut depth = 0usize;
    for character in latex.chars() {
        match character {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            c if c.is_control() && !c.is_whitespace() => return false,
            _ => {}
        }
        if depth > 64 {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::Math;

    #[test]
    fn cached_failures_still_consume_the_document_conversion_budget() {
        let mut math = Math::default();
        for _ in 0..256 {
            assert_eq!(math.render(r"\phantom{a+b}", false), None);
        }
        assert_eq!(math.render("x^2", false), None);
    }
}
