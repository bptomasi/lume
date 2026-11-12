//! Assessment of normalized publisher content, independent of HTTP fetching.
//!
//! Short text is not by itself a rejection. Explicit obstruction patterns
//! protect cached documents from being replaced by consent or legal pages.

use anyhow::{Result, bail};
use pulldown_cmark::{Event, Parser, TagEnd};

/// Explain suspicious results without claiming article completeness.
pub fn warnings(body: &str) -> Vec<&'static str> {
    let words = body.split_whitespace().count();
    let mut warnings = Vec::new();
    if words < 40 {
        warnings.push("Short extraction: fewer than 40 words");
    }
    if let Some(reason) = rejection(body) {
        warnings.push(reason);
    }
    if crate::content::body_kind(body) != crate::content::BodyKind::Text {
        warnings.push("No independent text; possible navigation-only page");
    }
    warnings
}

/// Reject clear obstructions while retaining legitimate short articles.
/// Length alone is never evidence that an article is unusable.
pub fn rejection(body: &str) -> Option<&'static str> {
    if body.split_whitespace().take(200).count() == 200 {
        return None;
    }
    let lower = assessment_text(body).to_lowercase();
    if ["accept all cookies", "manage consent", "enable javascript"]
        .iter()
        .any(|phrase| lower.contains(phrase))
    {
        return Some("Possible consent or browser-only page");
    }
    let footer_only = !lower.trim().is_empty()
        && lower
            .split("\n\n")
            .filter(|paragraph| !paragraph.trim().is_empty())
            .all(legal_notice);
    footer_only.then_some("Only a copyright or trademark footer was extracted")
}

fn assessment_text(body: &str) -> String {
    let mut text = String::new();
    for event in Parser::new(body) {
        match event {
            Event::Text(value) | Event::Code(value) => text.push_str(&value),
            Event::SoftBreak | Event::HardBreak => text.push(' '),
            Event::End(TagEnd::Paragraph | TagEnd::Heading(_)) => {
                text.push_str("\n\n");
            }
            _ => {}
        }
    }
    text
}

fn legal_notice(paragraph: &str) -> bool {
    let paragraph = paragraph.trim();
    let copyright = paragraph.starts_with("copyright")
        && (paragraph.contains('©')
            || paragraph.contains("(c)")
            || paragraph.contains("all rights reserved"));
    let trademark = paragraph
        .split(". ")
        .next()
        .is_some_and(|sentence| sentence.contains("is a registered trademark"));
    copyright || trademark
}

pub(crate) fn reject_obstruction(body: &str) -> Result<()> {
    if let Some(reason) = rejection(body) {
        bail!("Suspicious extraction: {reason}; previous content retained");
    }
    Ok(())
}
