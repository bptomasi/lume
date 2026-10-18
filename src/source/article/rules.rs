//! CSS parsing belongs to dom_query; extraction limits and quality belong here.
//!
//! A custom rule selects exactly one container. Ambiguous matches are rejected
//! rather than concatenated, which could duplicate nested articles/navigation.

use anyhow::{Result, bail, ensure};
use dom_query::{Document, Matcher};

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
    let lower = body.to_lowercase();
    if words < 200
        && ["accept all cookies", "manage consent", "enable javascript"]
            .iter()
            .any(|phrase| lower.contains(phrase))
    {
        warnings.push("Possible consent or browser-only page");
    }
    if crate::content::body_kind(body) != crate::content::BodyKind::Text {
        warnings.push("No independent text; possible navigation-only page");
    }
    warnings
}

pub(super) fn reject_obstruction(body: &str) -> Result<()> {
    if warnings(body)
        .iter()
        .any(|warning| warning.starts_with("Possible consent"))
    {
        bail!("Suspicious extraction: consent/browser page; original retained");
    }
    Ok(())
}
