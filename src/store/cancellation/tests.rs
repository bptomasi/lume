//! Cancellation preserves query-owned transactions and removes its callback.
use super::*;
use crate::cancellation::{Canceled, test_token};
use rusqlite::hooks::{AuthAction, AuthContext, Authorization};
use std::sync::atomic::Ordering;

#[test]
fn a_superseded_recursive_query_stops_and_the_connection_remains_usable()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("cancel.sqlite3"))?;
    let (generation, cancel) = test_token();
    store
        .connection
        .authorizer(Some(move |context: AuthContext<'_>| {
            if matches!(context.action, AuthAction::Recursive) {
                generation.store(1, Ordering::Release);
            }
            Authorization::Allow
        }))?;
    let result = store.read_cancellable(cancel, |store| {
        Ok(store.connection.query_row(
            include_str!("../sql/test_cancel_expensive_read.sql"),
            [],
            |row| row.get::<_, i64>(0),
        )?)
    });
    assert!(
        result
            .as_ref()
            .err()
            .is_some_and(|error| error.is::<Canceled>())
    );
    assert!(store.connection.is_autocommit());
    store
        .connection
        .authorizer(None::<fn(AuthContext<'_>) -> Authorization>)?;
    let resumed: i64 = store.connection.query_row(
        include_str!("../sql/test_cancel_resume_read.sql"),
        [],
        |row| row.get(0),
    )?;
    assert_eq!(resumed, 49_995_000);
    Ok(())
}

#[test]
fn cancellable_reads_leave_snapshot_ownership_with_the_query() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("ownership.sqlite3"))?;
    let (_, cancel) = test_token();
    store.read_cancellable(cancel, |store| {
        assert!(store.connection.is_autocommit());
        let transaction = store.connection.unchecked_transaction()?;
        let value: i64 = transaction.query_row(
            include_str!("../sql/test_cancel_resume_read.sql"),
            [],
            |row| row.get(0),
        )?;
        assert_eq!(value, 49_995_000);
        transaction.commit()?;
        Ok(())
    })?;
    assert!(store.connection.is_autocommit());
    Ok(())
}

#[test]
fn failed_reads_remove_the_callback_and_preserve_the_error() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("failed.sqlite3"))?;
    let (generation, cancel) = test_token();
    let error = store
        .read_cancellable::<()>(cancel, |_| {
            Err(anyhow::anyhow!("Controlled read failure"))
        })
        .err();
    assert!(error.is_some_and(|error| {
        error.to_string() == "Controlled read failure"
    }));
    assert_eq!(std::sync::Arc::strong_count(&generation), 1);
    generation.store(1, Ordering::Release);
    assert_reusable(&store)?;
    Ok(())
}

#[test]
fn unwinding_reads_remove_the_callback_before_connection_reuse() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("unwind.sqlite3"))?;
    let (generation, cancel) = test_token();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        store.read_cancellable::<()>(cancel, |_| {
            std::panic::resume_unwind(Box::new("Controlled read unwind"))
        })
    }));
    assert!(result.is_err());
    assert_eq!(std::sync::Arc::strong_count(&generation), 1);
    generation.store(1, Ordering::Release);
    assert_reusable(&store)?;
    Ok(())
}

fn assert_reusable(store: &Store) -> Result<()> {
    assert!(store.connection.is_autocommit());
    let value: i64 = store.connection.query_row(
        include_str!("../sql/test_cancel_resume_read.sql"),
        [],
        |row| row.get(0),
    )?;
    assert_eq!(value, 49_995_000);
    Ok(())
}
