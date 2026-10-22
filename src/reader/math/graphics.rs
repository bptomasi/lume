//! Optional worker-side typesetting; source text remains the annotation authority.
use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use std::sync::Arc;

use ratex_layout::{LayoutOptions, layout, to_display_list, validate_layout};
use ratex_parser::parser::parse;
use ratex_svg::{SvgColorSyntax, SvgOptions, render_to_svg_with_color_syntax};
use ratex_types::{
    color::Color,
    display_item::{DisplayItem, DisplayList},
    math_style::MathStyle,
};
use sha2::{Digest, Sha256};

/// A local, self-contained formula image; it never requests a publisher URL.
#[derive(Clone, Debug)]
pub struct FormulaImage {
    /// Original TeX content, without its Markdown delimiters.
    pub latex: Arc<str>,
    /// Immutable glyph outlines; no linked images, fonts or stylesheets.
    pub svg: Arc<[u8]>,
    /// Stable internal identity, independently validated from publisher URLs.
    pub identity: String,
    /// Reserved display rows, determined before images become visible.
    pub rows: usize,
}

pub(super) fn render(latex: &str, block: bool) -> Option<FormulaImage> {
    if !super::admissible(latex) {
        return None;
    }
    let ast = parse(latex).ok()?;
    let style = if block {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let options = LayoutOptions::default()
        .with_style(style)
        .with_color(Color::rgb(224.0 / 255.0, 219.0 / 255.0, 208.0 / 255.0));
    let tree = layout(&ast, &options);
    validate_layout(&tree).ok()?;
    let mut list = to_display_list(&tree);
    if list.items.len() > 2048
        || !admitted_fonts(&list.items)
        || !(0.0..=256.0).contains(&list.width)
        || !(0.0..=32.0).contains(&list.total_height())
    {
        return None;
    }
    let rows = compact_canvas(&mut list)?;
    let svg = render_to_svg_with_color_syntax(
        &list,
        &SvgOptions {
            font_size: 40.0,
            padding: 4.0,
            embed_glyphs: true,
            ..SvgOptions::default()
        },
        SvgColorSyntax::Rgb,
    );
    FormulaImage::from_svg(latex, svg, rows)
}

fn compact_canvas(list: &mut DisplayList) -> Option<usize> {
    // One em occupies 80% of a terminal row. Pad the canvas to complete rows
    // instead of stretching every short expression to fill its reserved slot.
    let rows = ((list.total_height() + 0.2) / 1.25).ceil().max(1.0) as usize;
    if rows > 16 {
        return None;
    }
    list.depth = rows as f64 * 1.25 - 0.2 - list.height;
    Some(rows)
}

impl FormulaImage {
    fn from_svg(latex: &str, svg: String, rows: usize) -> Option<Self> {
        (svg.len() <= 512 * 1024).then(|| Self {
            latex: Arc::from(latex),
            identity: format!(
                "lume-formula:{}",
                URL_SAFE_NO_PAD.encode(Sha256::digest(svg.as_bytes()))
            ),
            svg: Arc::from(svg.into_bytes()),
            rows,
        })
    }
}

fn admitted_fonts(items: &[DisplayItem]) -> bool {
    // Reject system fallback fonts before the exporter can discover them.
    // The bundled, unmodified KaTeX family covers admitted mathematical glyphs.
    items.iter().all(|item| match item {
        DisplayItem::GlyphPath { font, .. } => {
            !font.to_ascii_lowercase().contains("cjk")
                && !font.to_ascii_lowercase().contains("emoji")
        }
        _ => true,
    })
}
