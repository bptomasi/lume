//! Upgrade an actual V7 schema; a covering index must not rewrite owned data.

use anyhow::Result;
use rusqlite::Connection;

use crate::{model::View, store::Store};

use super::{MIGRATIONS, Migration, Runner};

#[test]
fn read_index_upgrade_preserves_v7_content_and_is_idempotent() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("upgrade.sqlite3");
    v7_library(&path)?;

    for _ in 0..2 {
        let store = Store::open(&path)?;
        assert_eq!(store.body(1)?, "Original body");
        assert!(store.article(1)?.starred);
        assert_eq!(store.comments(1)?[0].body, "Keep this private note");
        assert!(store.article_memberships(1)?.contains(&1));
        for view in [View::Inbox, View::Read, View::Trash] {
            assert_eq!(store.view_count(&view)?, 1, "{view:?}");
        }
        let plan: String = store.connection.query_row(
            r#"
            EXPLAIN QUERY PLAN
            SELECT count(*) FROM articles
            WHERE deleted = 0 AND unread = 1
            "#,
            [],
            |row| row.get(3),
        )?;
        assert!(plan.contains("COVERING INDEX articles_read_visibility"));
    }
    Ok(())
}

fn v7_library(path: &std::path::Path) -> Result<()> {
    let mut connection = Connection::open(path)?;
    connection.execute_batch(include_str!("../sql/create_schema.sql"))?;
    let migrations = MIGRATIONS
        .iter()
        .take(7)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../sql/test_setup_read_index_library.sql"
    ))?;
    Ok(())
}

#[test]
fn definition_upgrade_reserves_deleted_folder_identities() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("definitions.sqlite3");
    v7_library(&path)?;
    let mut connection = Connection::open(&path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(26)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../sql/test_setup_definition_identity_upgrade.sql"
    ))?;
    drop(connection);

    let store = Store::open(&path)?;
    assert_eq!(store.create_collection("After upgrade")?, 91);
    assert_eq!(store.body(1)?, "Original body");
    drop(store);
    let store = Store::open(&path)?;
    assert_eq!(store.create_collection("After reopen")?, 92);
    Ok(())
}

#[test]
fn notebook_upgrade_indexes_existing_notes_without_changing_owned_state()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("notebook-upgrade.sqlite3");
    v7_library(&path)?;
    let mut connection = Connection::open(&path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(8)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../sql/test_setup_notebook_upgrade.sql"
    ))?;
    drop(connection);

    for _ in 0..2 {
        let store = Store::open(&path)?;
        let page =
            store.search_notes("body:private tag:consensus", None, 0, 100)?;
        assert_eq!(page.total, 1);
        assert_eq!(page.notes[0].id, 1);
        let note = store.annotation(1, 1)?;
        assert_eq!(note.comment.body, "Keep this private note");
        assert_eq!(note.links, vec![2]);
        assert_eq!(note.review.map(|review| review.reviews), Some(3));
        assert_eq!(note.cards[0].id, 9);
        assert_eq!(note.cards[0].reviews, 2);
        assert_eq!(note.cards[0].due_at, 259300);
        assert_eq!(store.note_versions(1, 1)?.len(), 1);
        assert_eq!(store.article_memberships(1)?.len(), 1);
        assert_eq!(store.body(1)?, "Original body");
        assert!(store.article(1)?.starred);
    }
    Ok(())
}

#[test]
fn recommendations_adopt_v9_without_changing_articles_or_private_state()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("recommendation-upgrade.sqlite3");
    v7_library(&path)?;
    let mut connection = Connection::open(&path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(9)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    drop(connection);
    let store = Store::open(&path)?;
    assert_eq!(store.body(1)?, "Original body");
    assert!(store.article(1)?.starred);
    assert_eq!(store.comments(1)?[0].body, "Keep this private note");
    assert!(store.article_memberships(1)?.contains(&1));
    assert!(store.recommendations_dirty()?);
    let update = store.refresh_recommendations()?;
    assert_eq!(update.processed, 3);
    assert!(!update.remaining);
    drop(store);
    let reopened = Store::open(&path)?;
    assert!(!reopened.recommendations_dirty()?);
    assert_eq!(reopened.body(1)?, "Original body");
    Ok(())
}

#[test]
fn declarative_upgrade_preserves_legacy_interests_and_private_data()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("declarative-upgrade.sqlite3");
    v10_library(&path)?;

    for _ in 0..2 {
        let store = Store::open(&path)?;
        let profile = store.recommendation_profile()?;
        assert_eq!(profile.interests.len(), 9);
        let kubernetes = profile
            .interests
            .iter()
            .find(|interest| interest.name == "Kubernetes internals")
            .ok_or_else(|| anyhow::anyhow!("Legacy interest was lost"))?;
        assert_eq!(kubernetes.weight, 8);
        assert_eq!(store.comments(1)?[0].body, "Keep this private note");
        assert!(store.article_memberships(1)?.contains(&1));
        let config = directory.path().join("config.toml");
        crate::config::Session::open(&store, &config)?;
        let exported = crate::config::read_document(&config)?;
        let mut expected = profile.interests;
        expected.sort_by(|left, right| left.name.cmp(&right.name));
        assert_eq!(exported.recommendations.interests, expected);
    }
    Ok(())
}

#[test]
fn declarative_upgrade_never_replaces_an_explicit_legacy_profile() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("explicit-profile.sqlite3");
    v10_library(&path)?;
    let empty = crate::model::RecommendationProfile::default();
    let connection = Connection::open(&path)?;
    connection.execute(
        "INSERT INTO settings (key, value) VALUES ('recommendation_profile', ?1)",
        [serde_json::to_string(&empty)?],
    )?;
    drop(connection);

    let store = Store::open(&path)?;
    assert_eq!(store.recommendation_profile()?, empty);
    assert_eq!(store.body(1)?, "Original body");
    Ok(())
}

fn v10_library(path: &std::path::Path) -> Result<()> {
    v7_library(path)?;
    let mut connection = Connection::open(path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(10)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    Ok(())
}

#[test]
fn remote_cursor_upgrade_retains_identity_and_replays_incomplete_passes()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("remote-upgrade.sqlite3");
    v7_library(&path)?;
    let mut connection = Connection::open(&path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(18)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../sql/test_setup_remote_cursor_upgrade.sql"
    ))?;
    drop(connection);

    for _ in 0..2 {
        let store = Store::open(&path)?;
        let remote = store
            .feeds()?
            .remove(0)
            .server
            .ok_or_else(|| anyhow::anyhow!("Remote identity was lost"))?;
        assert_eq!(remote.sync_after_id, 0);
        assert_eq!(remote.last_changed, 100);
        assert_eq!(remote.sync_latest, 500);
        let mapped: i64 = store.connection.query_row(
            include_str!("../sql/servers/select_entry_mapping.sql"),
            (77, 88),
            |row| row.get(0),
        )?;
        assert_eq!(mapped, 1);
        assert!(store.article(1)?.starred);
        assert_eq!(store.body(1)?, "Original body");
        assert_eq!(store.comments(1)?[0].body, "Keep this private note");
    }
    Ok(())
}

#[test]
fn justification_upgrade_admits_unpublished_preferences_and_keeps_owned_data()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("justify-upgrade.sqlite3");
    let config = directory.path().join("config.toml");
    v12_justified_library(&path, &config)?;
    assert!(!crate::config::read_document(&config)?.ui.justify);

    for _ in 0..2 {
        let store = Store::open(&path)?;
        assert!(store.workspace()?.justify);
        assert_eq!(store.body(1)?, "Original body");
        assert_eq!(store.comments(1)?[0].body, "Keep this private note");
        store.reconcile_configuration(&config)?;
        assert!(crate::config::read_document(&config)?.ui.justify);
    }
    Ok(())
}

fn v12_justified_library(
    path: &std::path::Path,
    config: &std::path::Path,
) -> Result<()> {
    v10_library(path)?;
    let mut connection = Connection::open(path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(12)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    let legacy = Store {
        connection,
        pages: Default::default(),
    };
    legacy.save_workspace(&crate::model::Workspace::default())?;
    legacy.reconcile_configuration(config)?;
    let mut workspace = legacy.workspace()?;
    workspace.justify = true;
    legacy.save_workspace(&workspace)?;
    assert!(!legacy.configuration_pending(config)?);
    Ok(())
}

#[test]
fn retention_upgrade_adopts_legacy_dates_without_rewriting_owned_articles()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("retention-upgrade.sqlite3");
    v20_retention_library(&path)?;
    let store = Store::open(&path)?;
    let (last, adopted): (i64, i64) = store.connection.query_row(
        include_str!("../sql/test_select_retention_adoption.sql"),
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    assert_eq!(last, 3);
    let mut policy = crate::config::retention::Retention::default();
    policy.defaults.read_days = crate::config::retention::Age(Some(1));
    policy.defaults.unread_days = crate::config::retention::Age(Some(1));
    store.save_retention_policy(&policy)?;
    let images = directory.path().join("images");
    assert_eq!(
        crate::retention::preview(&store, &images, adopted)?.articles,
        0
    );
    assert_eq!(
        crate::retention::preview(&store, &images, adopted + 86_401)?.articles,
        2
    );
    assert_eq!(store.body(1)?, "Original body");
    assert_eq!(store.comments(1)?[0].body, "Keep this private note");
    assert_eq!(
        Store::open(&path)?.connection.query_row(
            include_str!("../sql/test_select_retention_adoption.sql"),
            [],
            |row| row.get::<_, i64>(1),
        )?,
        adopted
    );
    Ok(())
}

fn v20_retention_library(path: &std::path::Path) -> Result<()> {
    v7_library(path)?;
    let mut connection = Connection::open(path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(20)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../sql/test_prepare_retention_upgrade.sql"
    ))?;
    Ok(())
}

#[test]
fn image_bucket_upgrade_preserves_personal_data_and_rebuilds_only_in_maintenance()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("image-upgrade.sqlite3");
    indexed_v23_library(&path)?;
    let store = Store::open(&path)?;
    let before: (String, bool, i64) = store.connection.query_row(
        include_str!("../sql/test_select_image_bucket_upgrade.sql"),
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )?;
    assert_eq!(before, (String::new(), true, 6));
    assert!(store.refresh_image_references(
        &std::sync::atomic::AtomicBool::new(false)
    )?);
    let cache = crate::media::cache::path(
        std::path::Path::new(""),
        "https://images.example/upgrade",
    );
    let name = cache.to_string_lossy();
    for _ in 0..2 {
        let reopened = Store::open(&path)?;
        let indexed: (String, bool, i64) = reopened.connection.query_row(
            include_str!("../sql/test_select_image_bucket_upgrade.sql"),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(indexed, (name[..16].into(), false, 6));
        assert!(reopened.article(1)?.starred);
        assert_eq!(reopened.comments(1)?[0].body, "Keep this private note");
        assert!(reopened.article_memberships(1)?.contains(&1));
        assert_eq!(
            reopened.body(1)?,
            "![Kept](https://images.example/upgrade)"
        );
    }
    Ok(())
}

fn indexed_v23_library(path: &std::path::Path) -> Result<()> {
    v7_library(path)?;
    let mut connection = Connection::open(path)?;
    let migrations = MIGRATIONS
        .iter()
        .take(23)
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../sql/test_setup_image_bucket_upgrade.sql"
    ))?;
    Ok(())
}

#[test]
fn search_guard_upgrade_preserves_existing_content_flags_notes_and_progress()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("upgrade.sqlite3");
    let mut connection = Connection::open(&path)?;
    connection.execute_batch(include_str!("../sql/create_schema.sql"))?;
    let migrations = MIGRATIONS
        .iter()
        .take_while(|(name, _)| !name.starts_with("V28__"))
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection
        .execute_batch(include_str!("../sql/test_search_upgrade_data.sql"))?;
    drop(connection);
    let store = Store::open(&path)?;
    assert_eq!(store.body(1)?, "Old publisherword content.");
    let article = store.article(1)?;
    assert!(article.starred && !article.unread);
    assert_eq!(store.comments(1)?[0].body, "Kept private note");
    let full = store
        .cached_document(1)?
        .ok_or_else(|| anyhow::anyhow!("Full cache"))?;
    assert_eq!((full.revision, full.progress), (3, 42));
    assert!(full.body.contains("quartzcache"));
    assert_eq!(
        store
            .query_articles(&crate::model::ArticleQuery {
                view: crate::model::View::All,
                text: "quartzcache".into(),
                offset: 0,
                limit: 10,
                retained: None,
            })?
            .total,
        1
    );
    Ok(())
}
