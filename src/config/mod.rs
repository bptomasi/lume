//! Declarative configuration schemas and bounded standalone file operations.
//!
//! Portable configuration never contains article bodies or personal reading
//! data. Decoding and validation require no library or application state. The
//! configuration service reconciles these contracts with the SQLite projection.

pub mod backups;
pub(crate) mod document;
mod formatting;
pub(crate) mod merge;
pub(crate) mod publication;
pub mod retention;
mod schema;
mod update;

pub use schema::{
    Configuration, Feed, Folder, Recommendations, Server, SmartList, Ui,
};
pub(crate) use schema::{server_feed_reference, validate_env_name};
pub use update::Update;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Explicit conflict resolution; the rejected inputs are backed up first.
#[derive(Clone, Copy)]
pub enum Recovery {
    /// Apply the edited file while retaining articles, notes and memberships.
    File,
    /// Publish pending library definitions, replacing the conflicting file.
    Library,
}

pub(crate) fn absolute_path(path: &Path) -> Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let parent = path
        .parent()
        .context("Configuration requires a parent directory")?;
    publication::prepare_directory(parent)?;
    if path.symlink_metadata().is_ok() {
        return path
            .canonicalize()
            .context("Configuration path or symlink target is inaccessible");
    }
    Ok(parent.canonicalize()?.join(
        path.file_name()
            .context("Configuration requires a filename")?,
    ))
}

/// Parse and validate a portable TOML document without accessing the library.
///
/// # Errors
/// Returns bounded-size, syntax, version or semantic errors with context.
pub fn decode(text: &str) -> Result<Configuration> {
    anyhow::ensure!(
        text.len() <= 2 * 1024 * 1024,
        "Configuration exceeds 2 MiB"
    );
    let mut config: Configuration = toml_edit::de::from_str(text).map_err(
        |error: toml_edit::de::Error| {
            // The full TOML error display includes nearby source lines, which
            // may contain an inline credential unrelated to the invalid field.
            anyhow::anyhow!(
                "Invalid Lume TOML configuration: {}",
                error.message()
            )
        },
    )?;
    for feed in &mut config.feeds {
        feed.url = crate::urls::feed_url(&feed.url)?.to_string();
    }
    for server in &mut config.servers {
        server.url = crate::urls::server_url(&server.url)?;
    }
    config.normalize();
    config.validate()?;
    Ok(config)
}

/// Render a validated standalone configuration, suitable for backup or import.
///
/// # Errors
/// Returns validation or TOML serialization errors.
pub fn encode(config: &Configuration) -> Result<String> {
    config.validate()?;
    Ok(formatting::generated(config)?.to_string())
}

/// Preserve document comments while hiding inline credentials for diagnostics.
///
/// # Errors
/// Returns invalid configuration or TOML tree errors.
pub fn redacted_document(text: &str) -> Result<String> {
    let config = decode(text)?;
    document::render(Some(text), Some(&config), &config.redacted())
}

/// Read and validate one bounded UTF-8 document without opening a library.
///
/// # Errors
/// Returns missing-file, size, encoding, syntax or semantic errors.
pub fn read_document(path: &Path) -> Result<Configuration> {
    let text = publication::read(path)?.with_context(|| {
        format!("Configuration does not exist: {}", path.display())
    })?;
    decode(&text)
}

/// Export complete portable definitions privately, without replacing a file.
///
/// # Errors
/// Returns invalid configuration, existing-destination or filesystem failures.
pub fn export(config: &Configuration, path: &Path) -> Result<()> {
    let text = encode(config)?;
    crate::file_publication::atomic_file(path, |writer| {
        writer.write_all(text.as_bytes())?;
        Ok(())
    })
}

/// Format an existing TOML file atomically without opening a library.
///
/// # Errors
/// Rejects invalid data, inaccessible files, concurrent edits or changed values.
pub fn format_file(path: &Path) -> Result<()> {
    let path = absolute_path(path)?;
    let _lock = publication::lock(&path)?;
    let original = publication::read(&path)?
        .context("Configuration file does not exist")?;
    let formatted = formatting::format(&original)?;
    publication::publish(&path, Some(&original), &formatted)
}
