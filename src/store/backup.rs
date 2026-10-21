//! SQLite snapshots include committed WAL data and private reading state.
//!
//! Creation uses the online backup API under a read snapshot. Restore validates
//! an isolated copy before creating a new private destination; it never opens
//! the active library or replaces an existing directory. Images are disposable.

use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use rusqlite::{
    Connection, OpenFlags,
    backup::{Backup, StepResult},
};

use super::{Store, identity};

impl Store {
    /// Create a consistent SQLite snapshot without replacing an existing file.
    /// Declarative definitions and personal state come from the same snapshot.
    ///
    /// # Errors
    /// Returns filesystem, busy timeout or SQLite backup failures.
    pub fn backup_to(&self, output: &Path) -> Result<()> {
        self.read_snapshot(|store| publish_snapshot(&store.connection, output))
    }

    /// Snapshot an existing library read-only, preserving even its old schema.
    /// Neither configuration reconciliation nor migration runs on the source.
    ///
    /// # Errors
    /// Rejects missing, foreign, unversioned or unsupported source databases.
    pub fn snapshot_existing(source: &Path, output: &Path) -> Result<()> {
        let connection = Connection::open_with_flags(
            source,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        let transaction = connection.unchecked_transaction()?;
        identity::verify(&transaction)?;
        let version: i64 =
            transaction
                .query_row("PRAGMA user_version", [], |row| row.get(0))?;
        ensure!(version > 0, "Source does not contain a Lume library");
        publish_snapshot(&transaction, output)?;
        transaction.commit()?;
        Ok(())
    }

    /// Restore into a new directory containing library.sqlite3 and config.toml.
    /// The source is read-only; schema upgrades happen only in the staged copy.
    ///
    /// # Errors
    /// Rejects foreign/corrupt/newer snapshots and existing destinations.
    pub fn restore_snapshot(snapshot: &Path, output: &Path) -> Result<()> {
        ensure!(!output.try_exists()?, "Restore destination already exists");
        let source = Connection::open_with_flags(
            snapshot,
            OpenFlags::SQLITE_OPEN_READ_ONLY,
        )?;
        identity::verify(&source)?;
        let version: i64 =
            source.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        ensure!(version > 0, "Snapshot does not contain a Lume library");
        validate(&source)?;
        let parent = output
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        let staging = tempfile::tempdir_in(parent)?;
        let database = staging.path().join("library.sqlite3");
        super::prepare_path(&database)?;
        {
            let mut destination = Connection::open(&database)?;
            copy(&source, &mut destination)?;
        }
        prepare_restore(&database, staging.path())?;
        publish_restore(staging.path(), output)?;
        std::fs::File::open(parent)?.sync_all()?;
        Ok(())
    }
}

fn publish_snapshot(source: &Connection, output: &Path) -> Result<()> {
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut destination = Connection::open(temporary.path())?;
    copy(source, &mut destination)?;
    destination.pragma_update(None, "journal_mode", "DELETE")?;
    drop(destination);
    temporary.as_file().sync_all()?;
    temporary
        .persist_noclobber(output)
        .context("Backup destination already exists or cannot be written")?;
    std::fs::File::open(parent)?.sync_all()?;
    Ok(())
}

fn copy(source: &Connection, destination: &mut Connection) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(30);
    let backup = Backup::new(source, destination)?;
    loop {
        match backup.step(128)? {
            StepResult::Done => return Ok(()),
            StepResult::Busy | StepResult::Locked => {
                std::thread::sleep(Duration::from_millis(5));
            }
            _ => {}
        }
        ensure!(Instant::now() < deadline, "Snapshot exceeded 30 seconds");
    }
}

fn validate(connection: &Connection) -> Result<()> {
    let result: String =
        connection.query_row("PRAGMA quick_check", [], |row| row.get(0))?;
    ensure!(result == "ok", "Snapshot integrity check failed: {result}");
    let mut statement = connection.prepare("PRAGMA foreign_key_check")?;
    ensure!(
        statement.query([])?.next()?.is_none(),
        "Snapshot has broken references"
    );
    Ok(())
}

fn prepare_restore(database: &Path, directory: &Path) -> Result<()> {
    let store = Store::open(database)?;
    validate(&store.connection)?;
    let config = store.capture_configuration()?;
    crate::config::export(&config, &directory.join("config.toml"))?;
    store
        .connection
        .execute(include_str!("sql/update_restored_configuration.sql"), [])?;
    store
        .connection
        .pragma_update(None, "journal_mode", "DELETE")?;
    Ok(())
}

fn publish_restore(staging: &Path, output: &Path) -> Result<()> {
    // Exclusive directory creation reserves the destination without replacing
    // symlinks or existing directories. A later failure leaves it for inspection.
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(output)?;
    for name in ["library.sqlite3", "config.toml"] {
        std::fs::hard_link(staging.join(name), output.join(name))?;
        std::fs::File::open(output.join(name))?.sync_all()?;
    }
    std::fs::File::open(output)?.sync_all()?;
    Ok(())
}
