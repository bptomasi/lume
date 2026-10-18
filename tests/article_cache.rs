//! Full caches survive publisher changes and failures without losing user data.

mod common;

use anyhow::{Context, Result};
use lume::{
    model::{
        ArticleQuery, ArticleTarget, ContentSource, ExtractedDocument,
        FetchPolicy, Passage, View,
    },
    source,
    store::Store,
};

fn target(store: &Store) -> Result<ArticleTarget> {
    let article = store.library()?.articles.remove(0);
    Ok(ArticleTarget {
        article: article.id,
        url: article.url,
    })
}

fn document(body: &str) -> ExtractedDocument {
    ExtractedDocument {
        final_url: "https://journal.example/canonical/article".into(),
        body: body.into(),
    }
}

fn search_count(store: &Store, text: &str) -> Result<usize> {
    Ok(store
        .query_articles(&ArticleQuery {
            view: View::All,
            text: text.into(),
            offset: 0,
            limit: 10,
            retained: None,
        })?
        .total)
}

#[test]
fn full_cache_is_searchable_and_survives_reopen_without_changing_feed_or_notes()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    let feed_body = store.body(target.article)?;
    let list = store.create_collection("Deep reading")?;
    store.add_member(list, target.article)?;
    store.set_unread(target.article, false)?;
    store.set_starred(target.article, true)?;
    store.save_comment(target.article, None, "Keep this reasoning")?;
    store.clear_history()?;
    assert_eq!(search_count(&store, "quartzcache")?, 0);

    assert!(
        store
            .cache_document(&target, &document("The quartzcache mechanism."))?
    );

    assert_eq!(search_count(&store, "quartzcache")?, 1);
    assert_eq!(store.body(target.article)?, feed_body);
    assert!(!store.article(target.article)?.unread);
    assert!(store.article(target.article)?.starred);
    assert!(store.article_memberships(target.article)?.contains(&list));
    assert_eq!(
        store.comments(target.article)?[0].body,
        "Keep this reasoning"
    );
    assert!(!store.undo()?);
    drop(store);
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    let cache = reopened
        .cached_document(target.article)?
        .context("Cache missing")?;
    assert_eq!(cache.body, "The quartzcache mechanism.");
    assert_eq!(cache.url, target.url);
    assert_eq!(cache.final_url, "https://journal.example/canonical/article");
    assert_eq!(cache.revision, 1);
    assert!(cache.fetched_at > 0);
    Ok(())
}

#[test]
fn failed_refetch_preserves_successful_text_revision_and_progress() -> Result<()>
{
    let (_directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.cache_document(&target, &document("Cached failure evidence."))?;
    store.save_document_progress(target.article, 1, 7)?;
    let before = store
        .cached_document(target.article)?
        .context("Cache missing")?;

    store.document_failure(&target, "HTTP 503\u{1b}[31m unavailable")?;

    let after = store
        .cached_document(target.article)?
        .context("Cache missing")?;
    assert_eq!(after.body, before.body);
    assert_eq!(after.url, before.url);
    assert_eq!(after.revision, 1);
    assert_eq!(after.progress, 7);
    assert_eq!(after.fetched_at, before.fetched_at);
    assert_eq!(after.attempt_url, target.url);
    assert!(!after.error.contains('\u{1b}'));
    assert!(after.error.contains("503"));
    assert_eq!(search_count(&store, "evidence")?, 1);
    Ok(())
}

#[test]
fn identical_refetch_keeps_revision_but_changed_text_resets_its_own_progress()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.save_progress(target.article, 13)?;
    store.cache_document(&target, &document("First cached version."))?;
    store.save_document_progress(target.article, 1, 9)?;

    store.cache_document(&target, &document("First cached version."))?;
    let same = store
        .cached_document(target.article)?
        .context("Cache missing")?;
    assert_eq!(same.revision, 1);
    assert_eq!(same.progress, 9);
    store.cache_document(&target, &document("Second cached version."))?;
    store.save_document_progress(target.article, 1, 100)?;
    let changed = store
        .cached_document(target.article)?
        .context("Cache missing")?;
    assert_eq!(changed.revision, 2);
    assert_eq!(changed.progress, 0);
    assert_eq!(store.progress(target.article)?, 13);
    Ok(())
}

#[test]
fn publisher_url_change_invalidates_cache_search_and_rejects_stale_results()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let original = target(&store)?;
    store.cache_document(
        &original,
        &document("Unique stalecacheword content."),
    )?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?.articles;
    incoming[0].url = "https://journal.example/revised-article".into();

    store.ingest(&feed, "Systems Journal", &incoming)?;

    assert_eq!(search_count(&store, "stalecacheword")?, 0);
    assert!(!store.cache_document(&original, &document("Wrong replacement."))?);
    assert!(!store.document_failure(&original, "Stale failure")?);
    let retained = store
        .cached_document(original.article)?
        .context("Cache missing")?;
    assert_eq!(retained.url, original.url);
    assert_eq!(retained.body, "Unique stalecacheword content.");
    let current = target(&store)?;
    store
        .cache_document(&current, &document("New currentcacheword content."))?;
    assert_eq!(search_count(&store, "currentcacheword")?, 1);
    assert_eq!(
        store
            .cached_document(current.article)?
            .context("Cache missing")?
            .revision,
        2
    );
    Ok(())
}

#[test]
fn trash_rejects_delayed_downloads_without_destroying_an_earlier_cache()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.cache_document(&target, &document("Preserved cached article."))?;
    store.set_deleted(target.article, true)?;

    assert!(!store.cache_document(&target, &document("Late result."))?);
    assert!(!store.document_failure(&target, "Late failure")?);
    assert_eq!(
        store
            .cached_document(target.article)?
            .context("Cache missing")?
            .body,
        "Preserved cached article."
    );
    store.set_deleted(target.article, false)?;
    assert!(
        store
            .cache_document(&target, &document("Restored article content."))?
    );
    Ok(())
}

#[test]
fn invalid_document_never_replaces_a_good_cache() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.cache_document(&target, &document("Known good content."))?;
    for body in [
        String::new(),
        "[Comments](https://example.org)".into(),
        "bad\u{1b}content".into(),
        "x".repeat(524_289),
    ] {
        assert!(store.cache_document(&target, &document(&body)).is_err());
    }
    let mut invalid = document("Readable text.");
    invalid.final_url = "javascript:alert(1)".into();
    assert!(store.cache_document(&target, &invalid).is_err());
    assert_eq!(
        store
            .cached_document(target.article)?
            .context("Cache missing")?
            .body,
        "Known good content."
    );
    Ok(())
}

#[test]
fn full_passages_keep_origin_across_refresh_delete_and_undo() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.cache_document(&target, &document("A precise immutable passage."))?;
    let passage = Passage {
        quote: "precise immutable".into(),
        anchor: 2,
        source: ContentSource::Full,
        revision: 1,
    };
    store.save_passage(target.article, &passage, "My interpretation")?;
    let note = store.comments(target.article)?.remove(0);
    store.delete_comment(target.article, note.id)?;
    assert!(store.undo()?);
    store.cache_document(
        &target,
        &document("A revised precise immutable passage."),
    )?;

    assert!(
        store
            .save_passage(target.article, &passage, "Stale draft")
            .is_err()
    );
    let retained = store.comments(target.article)?.remove(0);
    assert_eq!(retained.quote, "precise immutable");
    assert_eq!(retained.body, "My interpretation");
    assert_eq!(retained.source, ContentSource::Full);
    assert_eq!(retained.source_revision, 1);
    Ok(())
}

#[test]
fn source_policy_is_durable_and_independent_of_http_validators_and_history()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    assert_eq!(store.fetch_policy(feed.id)?, FetchPolicy::Missing);
    store.set_fetch_policy(feed.id, FetchPolicy::Always)?;
    assert_eq!(store.feeds()?[0].etag, feed.etag);
    assert_eq!(store.feeds()?[0].modified, feed.modified);
    assert!(!store.undo()?);
    drop(store);
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.fetch_policy(feed.id)?, FetchPolicy::Always);
    reopened.set_fetch_policy(feed.id, FetchPolicy::Manual)?;
    assert_eq!(reopened.fetch_policy(feed.id)?, FetchPolicy::Manual);
    assert!(reopened.set_fetch_policy(-1, FetchPolicy::Always).is_err());
    Ok(())
}

#[test]
fn markdown_export_keeps_both_content_sources_and_quote_provenance()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.cache_document(&target, &document("A distinct cached document."))?;
    store.save_passage(
        target.article,
        &Passage {
            quote: "distinct".into(),
            anchor: 2,
            source: ContentSource::Full,
            revision: 1,
        },
        "Remember this",
    )?;
    let output = directory.path().join("article.md");

    lume::exchange::markdown::article(&store, target.article, &output)?;

    let text = std::fs::read_to_string(output)?;
    assert!(text.contains("A distinct cached document."));
    assert!(text.contains("## Feed content"));
    assert!(text.contains("Keep **one** copy"));
    assert!(text.contains("Quote source: Full article, revision 1"));
    assert!(text.contains("> distinct"));
    assert!(text.contains("Remember this"));
    Ok(())
}

#[test]
fn failed_document_transaction_rolls_back_cache_and_search_together()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    let connection =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    connection.execute_batch(include_str!(
        "../src/store/sql/test_create_failing_document_trigger.sql"
    ))?;

    assert!(
        store
            .cache_document(
                &target,
                &document("Atomic rollbackkeyword article.")
            )
            .is_err()
    );

    assert!(store.cached_document(target.article)?.is_none());
    assert_eq!(search_count(&store, "rollbackkeyword")?, 0);
    assert!(store.body(target.article)?.contains("Keep **one** copy"));
    Ok(())
}

#[test]
fn pre_upgrade_comment_undo_payloads_default_to_feed_provenance() -> Result<()>
{
    let (directory, store, _feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.save_highlight(target.article, "one", 0, "Original note")?;
    let id = store.comments(target.article)?[0].id;
    store.save_comment(target.article, Some(id), "Changed note")?;
    let connection =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    connection.execute_batch(include_str!(
        "../src/store/sql/test_update_legacy_comment_history.sql"
    ))?;

    assert!(store.undo()?);

    let note = store.comments(target.article)?.remove(0);
    assert_eq!(note.body, "Original note");
    assert_eq!(note.quote, "one");
    assert_eq!(note.source, ContentSource::Feed);
    assert_eq!(note.source_revision, 0);
    Ok(())
}

#[test]
fn a_feed_body_refresh_retains_the_full_body_search_index() -> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let target = target(&store)?;
    store.cache_document(
        &target,
        &document("A separate fullindexword mechanism."),
    )?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?.articles;
    incoming[0].body = "The new publisher feedrevisionword summary.".into();

    store.ingest(&feed, "Revised journal title", &incoming)?;

    assert_eq!(search_count(&store, "fullindexword")?, 1);
    assert_eq!(search_count(&store, "feedrevisionword")?, 1);
    assert_eq!(
        store
            .cached_document(target.article)?
            .context("Cache lost")?
            .revision,
        1
    );
    assert_eq!(
        store.body(target.article)?,
        "The new publisher feedrevisionword summary."
    );
    Ok(())
}
