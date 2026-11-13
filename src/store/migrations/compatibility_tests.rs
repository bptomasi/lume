//! Immutable fixtures were written by the previously shipped SQLite/Refinery.

use std::path::Path;

use anyhow::Result;
use rusqlite::Connection;

use crate::{model::ArticleQuery, store::Store};

use super::{LATEST_SCHEMA_VERSION, MIGRATIONS, Migration};

type History = Vec<(i64, String, String, String)>;

const V7: &[u8] =
    include_bytes!("../../../tests/fixtures/sqlite-3.46.0/v7.sqlite3");
const V27: &[u8] =
    include_bytes!("../../../tests/fixtures/sqlite-3.46.0/v27.sqlite3");

fn history(path: &Path, version: i64) -> Result<History> {
    let connection = Connection::open(path)?;
    let rows = connection
        .prepare(include_str!("../sql/migrations/test_select_history.sql"))?
        .query_map([version], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<rusqlite::Result<_>>()?;
    Ok(rows)
}

fn expected_history(version: i64) -> Result<History> {
    let json = if version == 7 {
        include_str!("../../../tests/fixtures/sqlite-3.46.0/v7-history.json")
    } else {
        include_str!("../../../tests/fixtures/sqlite-3.46.0/v27-history.json")
    };
    Ok(serde_json::from_str(json)?)
}

#[test]
fn shipped_refinery_checksums_and_history_survive_the_engine_upgrade()
-> Result<()> {
    assert!(rusqlite::version_number() >= 3_054_001);
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("old.sqlite3");
    for (bytes, version) in [(V7, 7), (V27, 27)] {
        std::fs::write(&path, bytes)?;
        let expected = expected_history(version)?;
        assert_eq!(history(&path, version)?, expected);
        for ((name, sql), row) in MIGRATIONS.iter().zip(&expected) {
            let migration = Migration::unapplied(name, sql)?;
            assert_eq!(migration.checksum().to_string(), row.3);
        }
        for _ in 0..2 {
            let store = Store::open(&path)?;
            assert_personal_state(&store, version)?;
            assert_eq!(history(&path, version)?, expected);
            assert_eq!(
                history(&path, LATEST_SCHEMA_VERSION)?.len(),
                MIGRATIONS.len()
            );
        }
    }
    Ok(())
}

fn assert_personal_state(store: &Store, version: i64) -> Result<()> {
    store.connection.execute_batch(include_str!(
        "../sql/migrations/test_check_search_integrity.sql"
    ))?;
    let article = store.article(1)?;
    assert!(article.starred);
    if version == 7 {
        assert!(article.unread);
        assert_eq!(store.body(1)?, "Original body");
        assert_eq!(store.comments(1)?[0].body, "Keep this private note");
        assert!(store.article_memberships(1)?.contains(&1));
    } else {
        assert!(!article.unread);
        assert_eq!(store.body(1)?, "Old publisherword content.");
        assert_eq!(store.comments(1)?[0].body, "Kept private note");
        let full = store.cached_document(1)?.ok_or_else(|| {
            anyhow::anyhow!("Upgrade lost the full article document")
        })?;
        assert_eq!((full.revision, full.progress), (3, 42));
        assert_eq!(
            store
                .query_articles(&ArticleQuery {
                    text: "quartzcache".into(),
                    view: crate::model::View::All,
                    offset: 0,
                    limit: 10,
                    retained: None,
                })?
                .total,
            1
        );
    }
    Ok(())
}

#[test]
fn invalid_unsigned_offsets_fail_without_wrapping_or_changing_owned_data()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("offsets.sqlite3");
    std::fs::write(&source, V27)?;
    let store = Store::open(&source)?;
    let before = store.progress(1)?;
    if let Ok(above_sqlite_max) = usize::try_from(1_u64 << 63) {
        assert!(store.save_progress(1, above_sqlite_max).is_err());
        assert_eq!(store.progress(1)?, before);
    }
    store.connection.execute_batch(include_str!(
        "../sql/migrations/test_negative_owned_offsets.sql"
    ))?;
    for error in [store.comments(1).err(), store.progress(1).err()] {
        let error = error.ok_or_else(|| {
            anyhow::anyhow!("A negative offset must fail unsigned decoding")
        })?;
        assert!(!crate::store::is_contention(&error));
        assert!(error.is::<rusqlite::Error>());
    }
    assert_eq!(store.body(1)?, "Old publisherword content.");
    assert!(store.article(1)?.starred);
    Ok(())
}

#[test]
fn old_engine_snapshots_restore_without_rewriting_the_source() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("old.sqlite3");
    for (bytes, version) in [(V7, 7), (V27, 27)] {
        std::fs::write(&source, bytes)?;
        let output = directory.path().join(format!("restored-{version}"));
        Store::restore_snapshot(&source, &output)?;
        assert_eq!(std::fs::read(&source)?, bytes);
        let restored = output.join("library.sqlite3");
        let store = Store::open(&restored)?;
        assert_personal_state(&store, version)?;
        assert_eq!(history(&restored, version)?, expected_history(version)?);
        let checkpoint = directory.path().join(format!("backup-{version}"));
        store.backup_to(&checkpoint)?;
        assert_personal_state(&Store::open(&checkpoint)?, version)?;
        assert!(Store::restore_snapshot(&source, &output).is_err());
        assert!(store.backup_to(&checkpoint).is_err());
    }
    Ok(())
}

#[test]
fn corrupt_shipped_history_is_rejected_by_open_and_isolated_restore()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("corrupt.sqlite3");
    std::fs::write(&source, V27)?;
    Connection::open(&source)?.execute_batch(include_str!(
        "../sql/migrations/test_corrupt_checksum.sql"
    ))?;
    let before = std::fs::read(&source)?;
    let history_before = history(&source, 27)?;
    let output = directory.path().join("rejected");
    let error = Store::restore_snapshot(&source, &output).err();
    assert!(
        error.is_some_and(|error| format!("{error:#}").contains("different"))
    );
    assert_eq!(std::fs::read(&source)?, before);
    assert!(!output.exists());
    let error = Store::open(&source).err();
    assert!(
        error.is_some_and(|error| format!("{error:#}").contains("different"))
    );
    assert_eq!(history(&source, 27)?, history_before);
    Ok(())
}

#[test]
fn valid_history_representations_survive_prevalidation_without_rewriting()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("valid-history.sqlite3");
    std::fs::write(&source, V27)?;
    let original = history(&source, 27)?;
    // Refinery parses these representations before comparing numeric checksums.
    // The preflight must accept the same grammar, not enforce its own format.
    let date = "2026-11-13t09:00:00.123456789-03:00";
    let checksum = format!("+000{}", original[0].3);
    Connection::open(&source)?.execute(
        include_str!("../sql/migrations/test_replace_history_values.sql"),
        (date, checksum),
    )?;
    let expected = history(&source, 27)?;
    for _ in 0..2 {
        let store = Store::open(&source)?;
        assert_personal_state(&store, 27)?;
        assert_eq!(history(&source, 27)?, expected);
    }
    let output = directory.path().join("restored");
    Store::restore_snapshot(&source, &output)?;
    assert_eq!(history(&output.join("library.sqlite3"), 27)?, expected);
    Ok(())
}

#[test]
fn malformed_history_values_return_errors_before_refinery_parsing() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let source = directory.path().join("malformed.sqlite3");
    for (date, checksum, field) in [
        ("not a date", "1", "timestamp"),
        ("2026-11-13T12:00:00Z", "not a number", "checksum"),
        ("2026-11-13T12:00:00Z", "18446744073709551616", "checksum"),
    ] {
        std::fs::write(&source, V27)?;
        Connection::open(&source)?.execute(
            include_str!("../sql/migrations/test_replace_history_values.sql"),
            (date, checksum),
        )?;
        let before = std::fs::read(&source)?;
        let output = directory.path().join("rejected");
        let error = Store::restore_snapshot(&source, &output).err();
        assert!(error.is_some_and(|error| {
            format!("{error:#}").contains(&format!("Invalid migration {field}"))
        }));
        assert_eq!(std::fs::read(&source)?, before);
        assert!(!output.exists());
        let history_before = history(&source, 27)?;
        let error = Store::open(&source).err();
        assert!(error.is_some_and(|error| {
            format!("{error:#}").contains(&format!("Invalid migration {field}"))
        }));
        assert_eq!(history(&source, 27)?, history_before);
    }
    Ok(())
}
