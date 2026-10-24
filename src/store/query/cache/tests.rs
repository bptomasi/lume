use super::*;
use crate::{
    model::{FeedOrigin, IncomingArticle, View},
    store::Store,
};
use anyhow::Context;

fn fixture(path: &std::path::Path) -> Result<Store> {
    let mut store = Store::open(path)?;
    store.subscribe("https://pages.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let articles = (0..75)
        .map(|index| IncomingArticle {
            external_id: index.to_string(),
            title: format!("Systems {index}"),
            url: format!("https://pages.example/{index}"),
            author: String::new(),
            published: index / 3,
            body: "Systems body".into(),
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, "Pages", &articles)?;
    Ok(store)
}

fn query(offset: usize) -> ArticleQuery {
    ArticleQuery {
        view: View::All,
        text: String::new(),
        offset,
        limit: 10,
        retained: None,
    }
}

#[test]
fn cursors_match_offsets_in_both_directions_and_across_timestamp_ties()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("pages.sqlite3");
    let store = fixture(&path)?;
    let reference = Store::open(&path)?;
    for offset in [40, 50, 60, 50, 40, 30, 0, 10, 70, 80] {
        reference.pages.borrow_mut().entries.clear();
        let expected = reference.query_articles(&query(offset))?;
        let actual = store.query_articles(&query(offset))?;
        assert_eq!(actual.total, expected.total);
        assert_eq!(
            actual.articles.iter().map(|a| a.id).collect::<Vec<_>>(),
            expected.articles.iter().map(|a| a.id).collect::<Vec<_>>()
        );
    }
    Ok(())
}

#[test]
fn snapshot_versions_follow_external_commits_only_after_the_snapshot_ends()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("pages.sqlite3");
    let store = fixture(&path)?;
    let writer = Store::open(&path)?;
    store.read_snapshot(|store| {
        let before = epoch(&store.connection)?;
        writer.subscribe("https://second.example/feed")?;
        assert_eq!(epoch(&store.connection)?, before);
        Ok(())
    })?;
    let first = store.query_articles(&query(0))?;
    let id = first.articles.first().context("Fixture article")?.id;
    writer.set_deleted(id, true)?;
    assert_eq!(store.query_articles(&query(10))?.total, 74);
    Ok(())
}

#[test]
fn rolled_back_writes_never_publish_page_hints() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = fixture(&directory.path().join("pages.sqlite3"))?;
    assert_eq!(store.query_articles(&query(0))?.total, 75);
    let transaction = store.write_transaction()?;
    store
        .connection
        .execute(include_str!("../../sql/test_cancel_page_write.sql"), [])?;
    assert_eq!(store.query_articles(&query(0))?.total, 0);
    assert!(store.pages.borrow().entries.is_empty());
    transaction.rollback()?;
    assert_eq!(store.query_articles(&query(0))?.total, 75);
    Ok(())
}
