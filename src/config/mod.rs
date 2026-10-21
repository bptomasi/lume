//! Declarative preferences with a recoverable SQLite projection.
//!
//! The file is portable configuration; publisher content and personal reading
//! data never enter it. SQLite records pending UI edits and their last published
//! base. Reconciliation merges independent file/UI changes, rejects conflicts,
//! and atomically publishes before acknowledging the revision. A crash between
//! filesystem publication and SQLite commit is recovered by the next merge.

pub(crate) mod document;
pub(crate) mod merge;
pub(crate) mod publication;
mod schema;
mod worker;

pub use schema::{Configuration, Feed, Folder, Recommendations, SmartList, Ui};
pub use worker::{Update, Worker};

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::store::Store;

/// One library's explicit configuration path, independent of the current cwd.
pub struct Session {
    path: PathBuf,
    database: PathBuf,
    initial: Update,
}

/// Explicit conflict resolution; the rejected inputs are backed up first.
#[derive(Clone, Copy)]
pub enum Recovery {
    /// Apply the edited file while retaining articles, notes and memberships.
    File,
    /// Publish pending library definitions, replacing the conflicting file.
    Library,
}

/// Resolve a conflict explicitly, retaining private backups of both inputs.
///
/// # Errors
/// Returns invalid selected input, backup/publication, lock or SQLite failures.
pub fn recover(
    store: &Store,
    path: &Path,
    source: Recovery,
) -> Result<Vec<PathBuf>> {
    let path = absolute_path(path)?;
    store.recover_configuration(&path, source)
}

fn absolute_path(path: &Path) -> Result<PathBuf> {
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

impl Session {
    /// Adopt an existing library or reconcile a configured one before use.
    ///
    /// # Errors
    /// Rejects invalid files, conflicts, inaccessible paths and SQLite failures.
    pub fn open(store: &Store, path: &Path) -> Result<Self> {
        let path = absolute_path(path)?;
        let initial = store.reconcile_configuration(&path)?;
        Ok(Self {
            path,
            database: store.database_path()?,
            initial,
        })
    }

    /// The effective path, resolving existing symlinks without replacing them.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Successfully reconciled initial document; safe to cache before UI entry.
    pub fn initial(&self) -> &Update {
        &self.initial
    }

    /// Finish admitted definition edits; personal reading data is untouched.
    ///
    /// # Errors
    /// Returns conflicts, publication errors or SQLite failures. Pending edits
    /// remain durable and are never reported as successfully written to TOML.
    pub fn flush(&self) -> Result<()> {
        let store = Store::open(&self.database)?;
        store.configuration_pending(&self.path)?;
        store.reconcile_configuration(&self.path)?;
        Ok(())
    }

    /// Start a bounded, independent reconciliation lane for the interactive UI.
    ///
    /// # Errors
    /// Reports thread creation failures.
    pub fn start(&self) -> Result<Worker> {
        Worker::start(self.path.clone(), self.database.clone())
    }
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
    let mut config: Configuration = toml_edit::de::from_str(text)
        .context("Invalid Lume TOML configuration")?;
    for feed in &mut config.feeds {
        feed.url = crate::source::feed_url(&feed.url)?.to_string();
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
    Ok(toml_edit::ser::to_string_pretty(config)?)
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
    crate::exchange::atomic_file(path, |writer| {
        writer.write_all(text.as_bytes())?;
        Ok(())
    })
}
