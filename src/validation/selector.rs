//! Pure CSS selector validation, independent of HTTP and extraction state.

use anyhow::{Result, ensure};
use dom_query::Matcher;

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
