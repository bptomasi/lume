//! Bounded OPML exchange and atomic Markdown exports.
//!
//! XML parsing/escaping belongs to quick-xml. Subscription import validates
//! the entire document before a single transaction. Exports use private
//! temporary files and never overwrite an existing user file implicitly.

pub mod incremental;
pub mod markdown;
pub mod notes;
pub mod opml;

use std::path::Path;

use anyhow::Result;

pub(crate) use crate::file_publication::atomic_file;

/// Read a bounded UTF-8 import file.
///
/// # Errors
/// Returns size, encoding or filesystem errors.
pub fn read_import(path: &Path) -> Result<String> {
    use std::io::Read;

    let mut text = String::new();
    std::fs::File::open(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_string(&mut text)?;
    anyhow::ensure!(text.len() <= 4 * 1024 * 1024, "Import exceeds 4 MiB");
    Ok(text)
}
