//! Feed-body availability, without guessing completeness from article length.

use pulldown_cmark::{Event, Parser, Tag, TagEnd};

/// Legacy cached placeholder; retained so old libraries need no data rewrite.
pub const MISSING_BODY: &str =
    "This feed contains no article body. Open the original URL to read it.";

/// What a normalized feed body actually supplies, not its article completeness.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BodyKind {
    /// At least one readable text or code token outside a reference.
    Text,
    /// Links or images without independent readable text.
    ReferencesOnly,
    /// No readable content, including the old cached placeholder.
    #[default]
    Missing,
}

/// Inspect CommonMark tokens; a short summary still counts as readable text.
///
/// Link labels and image descriptions are references, not evidence of prose.
/// This does not determine whether a publisher supplied its complete article.
pub fn body_kind(body: &str) -> BodyKind {
    if body.trim() == MISSING_BODY {
        return BodyKind::Missing;
    }
    let mut depth = 0_usize;
    let mut references = false;
    for event in Parser::new(body) {
        match event {
            Event::Html(ref text) | Event::InlineHtml(ref text)
                if super::anchors::invisible(text) => {}
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => {
                references = true;
                depth += 1;
            }
            Event::End(TagEnd::Link | TagEnd::Image) => {
                depth = depth.saturating_sub(1);
            }
            Event::Text(text)
            | Event::Code(text)
            | Event::Html(text)
            | Event::InlineHtml(text)
                if depth == 0 && !text.trim().is_empty() =>
            {
                return BodyKind::Text;
            }
            _ => {}
        }
    }
    if references {
        BodyKind::ReferencesOnly
    } else {
        BodyKind::Missing
    }
}
