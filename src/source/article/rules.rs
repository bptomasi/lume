//! CSS selection adapts dom_query; content owns extraction assessment.
//!
//! A custom rule selects exactly one container. Ambiguous matches are rejected
//! rather than concatenated, which could duplicate nested articles/navigation.

use anyhow::{Result, ensure};
use dom_query::{Document, Matcher};

pub(crate) use crate::content::extraction::reject_obstruction;
pub use crate::content::extraction::{rejection, warnings};
pub use crate::validation::validate_selector;

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
