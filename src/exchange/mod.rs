//! Bounded OPML exchange and atomic Markdown exports.
//!
//! XML parsing/escaping belongs to quick-xml. Subscription import validates
//! the entire document before a single transaction. Exports use private
//! temporary files and never overwrite an existing user file implicitly.

pub mod incremental;
pub mod markdown;
pub mod opml;

use std::{io::Write, path::Path};

use anyhow::{Context, Result};

pub(crate) fn atomic_file(
    path: &Path,
    write: impl FnOnce(&mut dyn Write) -> Result<()>,
) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    write(temporary.as_file_mut())?;
    temporary.as_file_mut().sync_all()?;
    temporary
        .persist_noclobber(path)
        .map_err(|error| error.error)
        .context("Export destination already exists or cannot be written")?;
    Ok(())
}

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
