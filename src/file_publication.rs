//! Create-only file publication with private staging and durable bytes.
//!
//! The writer owns file contents. Publication never replaces an existing file;
//! errors discard the temporary file and preserve the destination.

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
