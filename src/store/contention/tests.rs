use super::is_contention;
use crate::store::Store;
use anyhow::{Context, Result};
use rusqlite::{Error, ffi};
use std::time::Duration;

fn failure(code: i32) -> Error {
    Error::SqliteFailure(ffi::Error::new(code), Some("SQLite failure".into()))
}

#[test]
fn contention_survives_context_and_nested_error_sources() {
    for code in [
        ffi::SQLITE_BUSY,
        ffi::SQLITE_LOCKED,
        ffi::SQLITE_BUSY_SNAPSHOT,
        ffi::SQLITE_LOCKED_SHAREDCACHE,
    ] {
        let error = anyhow::Error::new(failure(code))
            .context("Owned write")
            .context("Worker completion");
        assert!(is_contention(&error), "code {code}");
        let converted = Error::FromSqlConversionFailure(
            0,
            rusqlite::types::Type::Integer,
            Box::new(failure(code)),
        );
        let error = anyhow::Error::new(converted).context("Decode result");
        assert!(is_contention(&error), "nested code {code}");
    }
}

#[test]
fn ordinary_sqlite_application_and_io_errors_are_not_contention() {
    for code in [ffi::SQLITE_CONSTRAINT, ffi::SQLITE_ERROR, ffi::SQLITE_IOERR] {
        let error = anyhow::Error::new(failure(code)).context("Owned write");
        assert!(!is_contention(&error), "code {code}");
    }
    assert!(!is_contention(&anyhow::anyhow!("DatabaseBusy")));
    let error = anyhow::Error::new(std::io::Error::other("DatabaseLocked"));
    assert!(!is_contention(&error));
    assert!(!is_contention(&Error::QueryReturnedNoRows.into()));
}

fn fixture() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store
        .connection
        .execute_batch(include_str!("../sql/test_setup_retention.sql"))?;
    Ok((directory, store))
}

#[test]
fn a_busy_write_preserves_state_and_retries_after_rollback() -> Result<()> {
    let (directory, store) = fixture()?;
    let rival = Store::open(&directory.path().join("library.sqlite3"))?;
    store.connection.busy_timeout(Duration::ZERO)?;
    let transaction = rival.write_transaction()?;
    let error = store
        .set_unread(1, true)
        .err()
        .context("Expected busy write")?;
    assert!(is_contention(&error.context("Personal task")));
    assert!(!store.article(1)?.unread);
    assert!(store.connection.is_autocommit());
    transaction.rollback()?;
    assert!(!store.undo()?);
    store.set_unread(1, true)?;
    assert!(store.article(1)?.unread);
    assert!(store.undo()?);
    assert!(!store.article(1)?.unread);
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn a_history_fault_rolls_back_without_becoming_retryable() -> Result<()> {
    let (_directory, store) = fixture()?;
    store
        .connection
        .execute_batch(include_str!("../sql/test_fail_flag_history.sql"))?;
    let error = store
        .set_unread(1, true)
        .err()
        .context("Expected history fault")?;
    assert!(!is_contention(&error));
    assert!(format!("{error:#}").contains("Flag history write failed"));
    assert!(!store.article(1)?.unread);
    assert!(store.connection.is_autocommit());
    assert!(!store.undo()?);
    Ok(())
}
