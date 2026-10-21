//! Consistent committed snapshots and isolated, non-destructive recovery.

mod common;

use anyhow::Result;
use lume::{config, store::Store};

#[test]
fn snapshot_includes_wal_notes_and_restores_portable_configuration()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let config_path = directory.path().join("original.toml");
    let _session = config::Session::open(&store, &config_path)?;
    store.save_comment(article, None, "Committed recovery evidence")?;
    store.set_fetch_policy(feed.id, lume::model::FetchPolicy::Manual)?;
    let snapshot = directory.path().join("snapshot.sqlite3");

    store.backup_to(&snapshot)?;
    store.save_comment(article, None, "After the snapshot")?;
    let restored = directory.path().join("restored");
    Store::restore_snapshot(&snapshot, &restored)?;

    let recovered = Store::open(&restored.join("library.sqlite3"))?;
    assert_eq!(recovered.comments(article)?.len(), 1);
    assert_eq!(
        recovered.comments(article)?[0].body,
        "Committed recovery evidence"
    );
    assert_eq!(
        recovered.fetch_policy(feed.id)?,
        lume::model::FetchPolicy::Manual
    );
    let restored_config = restored.join("config.toml");
    let _session = config::Session::open(&recovered, &restored_config)?;
    assert_eq!(
        config::read_document(&restored_config)?.feeds[0].url,
        feed.url
    );
    assert_eq!(store.comments(article)?.len(), 2);
    assert!(store.backup_to(&snapshot).is_err());
    assert!(Store::restore_snapshot(&snapshot, &restored).is_err());
    Ok(())
}

#[test]
fn invalid_or_foreign_snapshots_leave_sources_and_destination_untouched()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    for bytes in [b"not SQLite".as_slice(), b"".as_slice()] {
        let snapshot = directory.path().join("bad.sqlite3");
        std::fs::write(&snapshot, bytes)?;
        let output = directory.path().join("restored");
        assert!(Store::restore_snapshot(&snapshot, &output).is_err());
        assert_eq!(std::fs::read(&snapshot)?, bytes);
        assert!(!output.exists());
    }
    Ok(())
}

#[test]
fn backup_cli_remains_available_when_external_configuration_is_invalid()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.save_comment(
        article,
        None,
        "Back up before repairing preferences",
    )?;
    let configuration = directory.path().join("broken.toml");
    std::fs::write(&configuration, "this is not valid TOML = [")?;
    let snapshot = directory.path().join("snapshot.sqlite3");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lume"))
        .arg("--database")
        .arg(directory.path().join("library.sqlite3"))
        .arg("--config")
        .arg(&configuration)
        .arg("backup")
        .arg(&snapshot)
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(Store::open(&snapshot)?.comments(article)?.len(), 1);
    assert_eq!(
        std::fs::read_to_string(configuration)?,
        "this is not valid TOML = ["
    );
    Ok(())
}

#[test]
fn backup_cli_preserves_legacy_schema_and_original_file_bytes() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("legacy.sqlite3");
    let database = rusqlite::Connection::open(&source)?;
    database
        .execute_batch(include_str!("../src/store/sql/create_schema.sql"))?;
    database.execute_batch(include_str!(
        "../src/store/sql/test_setup_install_library.sql"
    ))?;
    drop(database);
    let before = std::fs::read(&source)?;
    let snapshot = directory.path().join("before-upgrade.sqlite3");

    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lume"))
        .arg("--database")
        .arg(&source)
        .arg("backup")
        .arg(&snapshot)
        .output()?;

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(std::fs::read(&source)?, before);
    let copied = rusqlite::Connection::open_with_flags(
        &snapshot,
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let version: i64 =
        copied.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(version, 1);
    let note: String =
        copied.query_row("SELECT body FROM comments", [], |row| row.get(0))?;
    assert_eq!(note, "A personal annotation that must survive migration.");
    Ok(())
}
