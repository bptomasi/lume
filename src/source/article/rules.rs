//! CSS parsing belongs to dom_query; extraction limits and quality belong here.
//!
//! A custom rule selects exactly one container. Ambiguous matches are rejected
//! rather than concatenated, which could duplicate nested articles/navigation.

use anyhow::{Result, bail, ensure};
use dom_query::{Document, Matcher};
use pulldown_cmark::{Event, Parser, TagEnd};

/// Compile a bounded CSS selector without downloading or altering content.
///
/// # Errors
/// Returns excessive length, controls or invalid CSS selector errors.
pub fn validate_selector(selector: &str) -> Result<()> {
    ensure!(selector.len() <= 1024, "Selector exceeds 1 KiB");
    ensure!(
        !selector.chars().any(char::is_control),
        "Selector contains control characters"
    );
    if !selector.is_empty() {
        Matcher::new(selector)
            .map_err(|error| anyhow::anyhow!("Invalid selector: {error:?}"))?;
    }
    Ok(())
}

pub(super) fn select(html: &str, selector: &str) -> Result<String> {
    validate_selector(selector)?;
    let matcher = Matcher::new(selector)
        .map_err(|error| anyhow::anyhow!("Invalid selector: {error:?}"))?;
    let document = Document::from(html);
    ensure!(
        document.select("*").length() <= 20_000,
        "Article exceeds 20,000 elements"
    );
    let selected = document.select_matcher(&matcher);
    ensure!(
        selected.length() == 1,
        "Selector must match one container; matched {}",
        selected.length()
    );
    Ok(selected.html().to_string())
}

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
