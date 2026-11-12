//! Shared bounded names for user-authored layouts and article collections.

use anyhow::{Result, ensure};

pub(crate) fn valid_workspace_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 256
        && name.chars().count() <= 64
        && name.trim() == name
        && !name.chars().any(char::is_control)
}

pub(crate) fn validate_collection_name(name: &str) -> Result<&str> {
    let name = name.trim();
    ensure!(!name.is_empty(), "List name cannot be empty");
    ensure!(
        name.chars().count() <= 60,
        "List name exceeds 60 characters"
    );
    ensure!(!name.chars().any(char::is_control), "Invalid list name");
    Ok(name)
}
