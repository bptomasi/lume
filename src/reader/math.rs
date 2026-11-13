//! Bounded LaTeX conversion to selectable terminal text, outside drawing.
//!
//! math-core owns TeX semantics; MathML is projected to Unicode rather than
//! claiming pixel typesetting. Unsupported or excessive input stays verbatim.

#[cfg(feature = "math")]
use math_core::{LatexToMathML, MathCoreConfig, MathDisplay, MaxExpansions};
#[cfg(feature = "math")]
mod embellishments;
#[cfg(feature = "math")]
mod fractions;
pub(super) mod graphics;
mod presentation;
#[cfg(feature = "math")]
mod projection;
pub(super) use presentation::Text as FormulaText;
#[cfg(feature = "math")]
mod scripts;

#[derive(Default)]
pub(super) struct Math {
    #[cfg(feature = "math")]
    converter: Option<LatexToMathML>,
    #[cfg(feature = "math")]
    count: usize,
    #[cfg(feature = "math")]
    bytes: usize,
    #[cfg(feature = "math")]
    cache: std::collections::VecDeque<(String, bool, Option<FormulaText>)>,
    #[cfg(feature = "graphic-math")]
    graphic_cache: std::collections::VecDeque<(
        String,
        bool,
        Option<graphics::FormulaImage>,
    )>,
}

impl Math {
    #[cfg(feature = "math")]
    pub(super) fn render(
        &mut self,
        latex: &str,
        block: bool,
    ) -> Option<FormulaText> {
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

    #[cfg(feature = "math")]
    fn convert(&mut self, latex: &str, block: bool) -> Option<FormulaText> {
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
        (text.len() <= 8192).then(|| {
            FormulaText::new(document.root_element(), text.trim().to_owned())
        })
    }

    #[cfg(feature = "graphic-math")]
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

    #[cfg(not(feature = "math"))]
    pub(super) fn render(
        &mut self,
        _latex: &str,
        _block: bool,
    ) -> Option<FormulaText> {
        None
    }

    #[cfg(not(feature = "graphic-math"))]
    pub(super) fn graphic(
        &mut self,
        _latex: &str,
        _block: bool,
    ) -> Option<graphics::FormulaImage> {
        None
    }
}

#[cfg(feature = "math")]
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

#[cfg(all(test, feature = "math"))]
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
