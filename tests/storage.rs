//! Durable domain behavior, rollback and independent connections.

mod common;

use anyhow::{Result, anyhow};
use lume::{source, store::Store};

#[test]
fn refresh_preserves_annotations_lists_and_personal_state() -> Result<()> {
    let (directory, mut store, mut feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    let list = store.create_collection("Deep dives")?;
    store.add_member(list, id)?;
    store.set_unread(id, false)?;
    store.set_starred(id, true)?;
    store.save_comment(id, None, "Keep this idea")?;
    let parsed = source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    feed.etag = "v2".into();
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    drop(store);

    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    let library = reopened.library()?;
    assert_eq!(library.articles.len(), 1);
    assert!(!library.articles[0].unread);
    assert!(library.articles[0].starred);
    assert_eq!(library.articles[0].comments, 1);
    assert_eq!(
        library.collections[0]
            .articles
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![id]
    );
    assert_eq!(reopened.comments(id)?[0].body, "Keep this idea");
    assert_eq!(library.feeds[0].etag, "v2");
    Ok(())
}

#[test]
fn failed_ingestion_does_not_advance_http_validators() -> Result<()> {
    let (directory, mut store, mut feed) = common::seeded_store()?;
    let database =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    database.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_article.sql"
    ))?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].title = "Publisher correction must roll back".into();
    feed.etag = "must-not-commit".into();
    assert!(
        store
            .ingest(&feed, "Wrong title", &parsed.articles)
            .is_err()
    );
    assert_eq!(store.feeds()?[0].etag, "");
    assert_eq!(store.feeds()?[0].title, "Systems Journal");
    assert!(
        store
            .body(store.library()?.articles[0].id)?
            .contains("Keep **one**")
    );
    Ok(())
}

#[test]
fn list_lifecycle_retains_articles_and_comments() -> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    let list = store.create_collection("  Deep dives  ")?;
    assert_eq!(store.create_collection("deep DIVES")?, list);
    store.add_member(list, id)?;
    store.add_member(list, id)?;
    store.save_comment(id, None, "Private")?;
    store.rename_collection(list, "Systems")?;
    assert_eq!(
        store.library()?.collections[0]
            .articles
            .iter()
            .copied()
            .collect::<Vec<_>>(),
        vec![id]
    );
    store.remove_member(list, id)?;
    assert!(store.library()?.collections[0].articles.is_empty());
    store.delete_collection(list)?;
    store.unsubscribe(feed.id)?;
    assert!(store.feeds()?.is_empty());
    assert_eq!(store.library()?.articles.len(), 1);
    assert_eq!(store.comments(id)?.len(), 1);
    Ok(())
}

#[test]
fn comment_edits_require_the_owning_article_and_validate_content() -> Result<()>
{
    let (_directory, store, _feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    store.save_comment(id, None, "first")?;
    let comment = store
        .comments(id)?
        .first()
        .ok_or_else(|| anyhow!("Missing comment"))?
        .id;
    assert!(
        store
            .save_comment(id + 1, Some(comment), "wrong owner")
            .is_err()
    );
    assert!(store.save_comment(id, Some(comment), "   ").is_err());
    assert!(store.save_comment(id, None, &"x".repeat(16_385)).is_err());
    assert!(store.add_member(999, id).is_err());
    store.save_comment(id, Some(comment), "edited")?;
    assert_eq!(store.comments(id)?[0].body, "edited");
    store.delete_comment(id, comment)?;
    assert!(store.comments(id)?.is_empty());
    Ok(())
}

#[test]
fn unknown_schema_is_rejected_without_resetting_data() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    drop(store);
    let path = directory.path().join("library.sqlite3");
    let database = rusqlite::Connection::open(&path)?;
    database.pragma_update(None, "user_version", 99)?;
    assert!(Store::open(&path).is_err());
    let count: i64 =
        database
            .query_row("SELECT count(*) FROM articles", [], |row| row.get(0))?;
    assert_eq!(count, 1);
    Ok(())
}

#[cfg(unix)]
#[test]
fn new_libraries_use_private_permissions() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let directory = tempfile::tempdir()?;
    let parent = directory.path().join("private");
    let path = parent.join("library.sqlite3");
    let _store = Store::open(&path)?;
    assert_eq!(
        std::fs::metadata(&parent)?.permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        std::fs::metadata(&path)?.permissions().mode() & 0o777,
        0o600
    );
    Ok(())
}

#[test]
fn foreign_databases_are_rejected_without_changing_their_bytes() -> Result<()> {
    let directory = tempfile::tempdir()?;
    for (version, application) in [(0, 0), (1, 0), (0, 42)] {
        let path = directory
            .path()
            .join(format!("foreign-{version}-{application}.db"));
        let database = rusqlite::Connection::open(&path)?;
        database.execute_batch("CREATE TABLE unrelated (value TEXT);")?;
        database.pragma_update(None, "user_version", version)?;
        database.pragma_update(None, "application_id", application)?;
        drop(database);
        let before = std::fs::read(&path)?;

        assert!(Store::open(&path).is_err());

        assert_eq!(std::fs::read(&path)?, before);
        assert!(!path.with_extension("db-wal").exists());
        assert!(!path.with_extension("db-shm").exists());
    }
    Ok(())
}

#[test]
fn legacy_libraries_gain_an_identifier_without_losing_articles() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let before = store.library()?.articles;
    drop(store);
    let path = directory.path().join("library.sqlite3");
    let database = rusqlite::Connection::open(&path)?;
    database.pragma_update(None, "application_id", 0)?;
    drop(database);

    let store = Store::open(&path)?;

    assert_eq!(store.library()?.articles[0].id, before[0].id);
    let database = rusqlite::Connection::open(&path)?;
    let application: i64 =
        database.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    assert_eq!(application, 0x4c55_4d45);
    Ok(())
}
