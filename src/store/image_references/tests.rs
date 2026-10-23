//! Generations survive queue completion and protect against stale concurrent work.
use super::*;
use anyhow::Context;
fn library() -> Result<(tempfile::TempDir, Store, crate::model::Feed)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://journal.test/feed")?;
    let feed = store.feeds()?.remove(0);
    publish(&mut store, &feed, "![Image](old.svg)")?;
    Ok((directory, store, feed))
}
fn publish(
    store: &mut Store,
    feed: &crate::model::Feed,
    body: &str,
) -> Result<()> {
    let mut parsed = crate::source::parse(
        include_bytes!("../../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    parsed.articles[0].url = "https://journal.test/articles/story".into();
    parsed.articles[0].body = body.into();
    store.ingest(feed, &parsed.title, &parsed.articles)
}
fn urls(store: &Store) -> Result<Vec<String>> {
    let mut urls = Vec::new();
    store.visit_image_urls(&AtomicBool::new(false), |url| {
        urls.push(url.into());
        Ok(())
    })?;
    Ok(urls)
}
#[test]
fn obsolete_generations_cannot_replace_a_new_source_after_completion()
-> Result<()> {
    let (_directory, mut store, feed) = library()?;
    let first = store
        .image_reference_batch()?
        .pop()
        .context("First queue entry")?;
    let stale = store
        .image_reference_batch()?
        .pop()
        .context("Concurrent queue entry")?;
    store.publish_image_references(vec![index(
        first,
        &AtomicBool::new(false),
    )?])?;
    publish(&mut store, &feed, "![Image](new.svg)")?;
    store.publish_image_references(vec![index(
        stale,
        &AtomicBool::new(false),
    )?])?;
    assert!(!store.image_reference_batch()?.is_empty());
    assert!(store.refresh_image_references(&AtomicBool::new(false))?);
    assert_eq!(urls(&store)?, vec!["https://journal.test/articles/new.svg"]);
    assert!(store.image_reference_batch()?.is_empty());
    Ok(())
}
#[test]
fn cancellation_does_not_acknowledge_or_publish_partial_references()
-> Result<()> {
    let (_directory, store, _) = library()?;
    assert!(!store.refresh_image_references(&AtomicBool::new(true))?);
    assert!(urls(&store)?.is_empty());
    assert!(!store.image_reference_batch()?.is_empty());
    store.refresh_image_references(&AtomicBool::new(false))?;
    assert_eq!(urls(&store)?, vec!["https://journal.test/articles/old.svg"]);
    Ok(())
}

#[test]
fn indexing_failure_rolls_back_acknowledgement_and_full_deletion_invalidates_urls()
-> Result<()> {
    let (_directory, store, _) = library()?;
    let article = store.library()?.articles[0].id;
    store.connection.execute(
        include_str!("../sql/test_update_image_full.sql"),
        (article, "![Full](full.svg)"),
    )?;
    store.connection.execute_batch(include_str!(
        "../sql/test_reject_image_reference.sql"
    ))?;
    assert!(
        store
            .refresh_image_references(&AtomicBool::new(false))
            .is_err()
    );
    assert!(urls(&store)?.is_empty());
    assert!(!store.image_reference_batch()?.is_empty());
    store.connection.execute_batch(include_str!(
        "../sql/test_drop_image_reference_failure.sql"
    ))?;
    assert!(store.refresh_image_references(&AtomicBool::new(false))?);
    assert_eq!(
        urls(&store)?,
        vec![
            "https://cdn.test/page/full.svg",
            "https://journal.test/articles/old.svg"
        ]
    );
    store
        .connection
        .execute(include_str!("../sql/test_clear_image_full.sql"), [article])?;
    assert!(store.refresh_image_references(&AtomicBool::new(false))?);
    assert_eq!(urls(&store)?, vec!["https://journal.test/articles/old.svg"]);
    let changes = store.connection.total_changes();
    assert!(store.refresh_image_references(&AtomicBool::new(false))?);
    assert_eq!(store.connection.total_changes(), changes);
    Ok(())
}
