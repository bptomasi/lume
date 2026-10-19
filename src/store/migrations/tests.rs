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
