//! Reconcile portable configuration with one library's durable projection.
//!
//! This service owns sessions, filesystem observation and worker lifecycle.
//! Configuration decoding and validation remain independent of SQLite;
//! transactional projection and recovery remain owned by the store adapter.

mod worker;
pub use worker::Worker;

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::{
    config::{self, Recovery, Update},
    store::Store,
};

/// One library's explicit configuration path, independent of the current cwd.
pub struct Session {
    path: PathBuf,
    database: PathBuf,
    initial: Update,
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
    let path = config::absolute_path(path)?;
    store.recover_configuration(&path, source)
}

impl Session {
    /// Adopt an existing library or reconcile a configured one before use.
    ///
    /// # Errors
    /// Rejects invalid files, conflicts, inaccessible paths and SQLite failures.
    pub fn open(store: &Store, path: &Path) -> Result<Self> {
        let path = config::absolute_path(path)?;
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
