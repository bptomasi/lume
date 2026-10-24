//! Durable workspace contracts, including migration and library-wide queries.

mod common;

use std::collections::BTreeSet;

use anyhow::Result;
use lume::{
    model::{ArticleQuery, IncomingArticle, SmartFilter, View},
    store::Store,
};

fn query(view: View, text: &str, offset: usize, limit: usize) -> ArticleQuery {
    ArticleQuery {
        view,
        text: text.into(),
        offset,
        limit,
        retained: None,
    }
}

#[test]
fn legacy_migration_preserves_personal_data_and_backfills_search() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("legacy.sqlite3");
    let connection = rusqlite::Connection::open(&path)?;
    connection
        .execute_batch(include_str!("../src/store/sql/create_schema.sql"))?;
    connection.execute_batch(include_str!(
        "../src/store/sql/test_setup_legacy_workspace.sql"
    ))?;
    drop(connection);

    let store = Store::open(&path)?;

    let article = store.article(42)?;
    assert!(article.starred);
    assert!(!article.unread);
    assert_eq!(store.comments(42)?[0].created, 1234);
    assert_eq!(store.article_memberships(42)?, BTreeSet::from([3]));
    assert_eq!(
        store
            .query_articles(&query(View::All, "cafe", 0, 10))?
            .total,
        1
    );
    assert_eq!(
        store
            .query_articles(&query(View::Annotated, "insight", 0, 10))?
            .total,
        1
    );
    drop(store);
    assert_eq!(Store::open(&path)?.article(42)?.title, "Old article");
    Ok(())
}

#[test]
fn notes_are_searchable_after_edit_delete_and_persistent_undo() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    store.save_comment(id, None, "A capybara insight")?;
    let comment = store.comments(id)?[0].id;
    assert_eq!(
        store
            .query_articles(&query(View::All, "capybara", 0, 10))?
            .total,
        1
    );

    store.save_comment(id, Some(comment), "A quokka insight")?;
    assert_eq!(
        store
            .query_articles(&query(View::All, "capybara", 0, 10))?
            .total,
        0
    );
    store.delete_comment(id, comment)?;
    assert_eq!(
        store
            .query_articles(&query(View::Annotated, "", 0, 10))?
            .total,
        0
    );
    drop(store);

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(store.undo()?);
    assert_eq!(store.comments(id)?[0].body, "A quokka insight");
    assert!(store.undo()?);
    assert_eq!(store.comments(id)?[0].body, "A capybara insight");
    Ok(())
}

#[test]
fn multi_list_change_rolls_back_invalid_targets_and_undo_restores_deletion()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    let first = store.create_collection("Research")?;
    let second = store.create_collection("Weekend")?;
    store.set_memberships(id, &BTreeSet::from([first, second]))?;
    store.clear_history()?;

    assert!(
        store
            .set_memberships(id, &BTreeSet::from([first, 9999]))
            .is_err()
    );
    assert_eq!(
        store.article_memberships(id)?,
        BTreeSet::from([first, second])
    );
    assert!(!store.undo()?);
    store.delete_collection(first)?;
    assert_eq!(store.article_memberships(id)?, BTreeSet::from([second]));
    assert!(store.undo()?);
    assert_eq!(
        store.article_memberships(id)?,
        BTreeSet::from([first, second])
    );
    Ok(())
}

#[test]
fn highlights_survive_publisher_replacement_and_flags_have_independent_views()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    store.save_highlight(
        id,
        "Keep **one** copy",
        0,
        "Compare with the new text",
    )?;
    store.set_queued(id, true)?;
    store.set_starred(id, true)?;
    store.set_unread(id, false)?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = "Publisher replaced the text".into();

    store.ingest(&feed, "Revised journal", &parsed.articles)?;

    assert_eq!(store.comments(id)?[0].quote, "Keep **one** copy");
    assert_eq!(
        store.query_articles(&query(View::Queue, "", 0, 10))?.total,
        1
    );
    assert_eq!(
        store.query_articles(&query(View::Inbox, "", 0, 10))?.total,
        0
    );
    assert_eq!(
        store
            .query_articles(&query(View::Starred, "", 0, 10))?
            .total,
        1
    );
    assert!(store.undo()?);
    assert!(store.article(id)?.unread);
    assert!(store.article(id)?.queued);
    Ok(())
}

#[test]
fn smart_lists_combine_criteria_and_invalid_search_is_rejected() -> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    store.set_starred(id, true)?;
    store.save_smart_list(
        "Saved thinking",
        &SmartFilter {
            text: "measure".into(),
            paused: false,
            unread: true,
            starred: true,
            feed: Some(feed.id),
            feeds: Vec::new(),
            days: None,
        },
    )?;
    let smart = store.smart_lists()?.remove(0);
    assert_eq!(
        store
            .query_articles(&query(View::Smart(smart.id), "", 0, 10))?
            .total,
        1
    );
    store.set_unread(id, false)?;
    assert_eq!(
        store
            .query_articles(&query(View::Smart(smart.id), "", 0, 10))?
            .total,
        0
    );
    assert!(
        store
            .query_articles(&query(View::All, "\" OR *", 0, 10))
            .is_err()
    );
    for text in ["NEAR(body)", "title:cache", "'; DROP TABLE articles;--"] {
        let _ = store.query_articles(&query(View::All, text, 0, 10))?;
    }
    assert!(store.article(id)?.starred);
    Ok(())
}

#[test]
fn old_favorites_and_annotations_remain_accessible_beyond_ten_thousand()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let oldest = store.library()?.articles[0].id;
    store.set_starred(oldest, true)?;
    store.set_unread(oldest, false)?;
    store.save_comment(oldest, None, "An ancient discovery")?;
    for batch in 0..6 {
        let articles = (0..2000)
            .map(|index| IncomingArticle {
                body_origin: lume::model::FeedOrigin::Feed,
                external_id: format!("{batch}-{index}"),
                title: format!("Newer {batch}-{index}"),
                url: String::new(),
                author: String::new(),
                published: 2_000_000_000 + batch * 2000 + index,
                body: "A bounded page".into(),
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, &feed.title, &articles)?;
    }

    let page = store.query_articles(&query(View::All, "", 12_000, 200))?;

    let read = store.query_articles(&query(View::Read, "", 0, 10))?;
    assert_eq!(read.total, 1);
    assert_eq!(read.articles[0].id, oldest);

    assert_eq!(page.total, 12_001);
    assert_eq!(page.articles[0].id, oldest);
    assert_eq!(
        store
            .query_articles(&query(View::Starred, "", 0, 10))?
            .articles[0]
            .id,
        oldest
    );
    assert_eq!(
        store
            .query_articles(&query(View::Annotated, "ancient", 0, 10))?
            .articles[0]
            .id,
        oldest
    );
    Ok(())
}
