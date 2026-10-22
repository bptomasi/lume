//! Bounded LaTeX conversion to selectable terminal text, outside drawing.
//!
//! math-core owns TeX semantics; MathML is projected to Unicode rather than
//! claiming pixel typesetting. Unsupported or excessive input stays verbatim.

use math_core::{LatexToMathML, MathCoreConfig, MathDisplay, MaxExpansions};
mod projection;
mod scripts;

#[derive(Default)]
pub(super) struct Math {
    converter: Option<LatexToMathML>,
    count: usize,
    bytes: usize,
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
        if self.converter.is_none() {
            let config = MathCoreConfig {
                max_expansions: MaxExpansions(64),
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
