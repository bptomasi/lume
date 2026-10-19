//! Shared validation for user-owned collection names.

use anyhow::{Result, ensure};

pub(crate) fn collection_name(name: &str) -> Result<&str> {
    let name = name.trim();
    ensure!(!name.is_empty(), "List name cannot be empty");
    ensure!(
        name.chars().count() <= 60,
        "List name exceeds 60 characters"
    );
    ensure!(!name.chars().any(char::is_control), "Invalid list name");
    Ok(name)
}
