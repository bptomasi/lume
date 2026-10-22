//! Reading services preserve library identity and personal data across maintenance.

mod common;

use std::sync::atomic::AtomicBool;

use anyhow::{Context, Result};
use lume::{
    model::{
        ArticleTarget, ExtractedDocument, OfflineLimits, OfflineStatus,
        SessionItem, View, WebDocument,
    },
    store::Store,
};

fn page(url: &str) -> WebDocument {
    WebDocument {
        url: url.into(),
        title: "A saved independent page".into(),
        body: "# A saved independent page\n\nUseful durable content.".into(),
    }
}

fn cache(store: &Store, id: i64, url: &str, body: &str) -> Result<()> {
    assert!(store.cache_document(
        &ArticleTarget {
            article: id,
            url: url.into(),
        },
        &ExtractedDocument {
            final_url: url.into(),
            body: body.into(),
        }
    )?);
    Ok(())
}

#[test]
fn saving_pages_is_explicit_idempotent_and_creates_no_subscription()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let store = Store::open(&path)?;
    assert!(store.feeds()?.is_empty());
    let document = page("https://example.org/page");

    let id = store.save_web_document(&document)?;
    assert_eq!(store.save_web_document(&document)?, id);
    assert!(store.feeds()?.is_empty());
    assert_eq!(store.library()?.articles.len(), 1);
    assert_eq!(store.body(id)?, document.body);
    assert_eq!(store.article_memberships(id)?.len(), 1);
    store.save_comment(id, None, "Preserve this note")?;
    store.set_starred(id, true)?;
    let mut replacement = document.clone();
    replacement.body = "Unexpected overwrite".into();
    assert_eq!(store.save_web_document(&replacement)?, id);
    assert_eq!(store.body(id)?, document.body);
    assert_eq!(store.comments(id)?.len(), 1);
    store.set_deleted(id, true)?;
    assert!(store.save_web_document(&document).is_err());
    Ok(())
}

#[test]
fn cleanup_removes_only_unprotected_full_text_and_updates_search() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    let original = store.body(article.id)?;
    cache(
        &store,
        article.id,
        &article.url,
        "Distinctive quartzcache text.",
    )?;
    let images = directory.path().join("images");
    lume::storage::clean(
        &store,
        &images,
        lume::storage::Cleanup::All,
        &AtomicBool::new(false),
    )?;

    assert!(
        store
            .cached_document(article.id)?
            .context("Cache")?
            .body
            .is_empty()
    );
    assert_eq!(store.body(article.id)?, original);
    assert!(store.article(article.id)?.unread);
    let result = store.query_articles(&lume::model::ArticleQuery {
        view: View::All,
        text: "quartzcache".into(),
        offset: 0,
        limit: 10,
        retained: None,
    })?;
    assert_eq!(result.total, 0);
    Ok(())
}

#[test]
fn offline_checkpoints_protect_full_text_and_resume_without_network()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    cache(&store, article.id, &article.url, "Full prepared content.")?;
    store.set_queued(article.id, true)?;
    let state =
        lume::offline::capture(&store, View::Queue, OfflineLimits::default())?;
    let images = directory.path().join("images");

    let state = lume::offline::prepare(
        &store,
        &images,
        state,
        &AtomicBool::new(false),
        |_| {},
    )?;

    assert_eq!(state.items[0].status, OfflineStatus::Ready);
    assert_eq!(state.items[0].bytes, 22);
    assert!(!state.running);
    store.set_queued(article.id, false)?;
    lume::storage::clean(
        &store,
        &images,
        lume::storage::Cleanup::All,
        &AtomicBool::new(false),
    )?;
    assert!(
        !store
            .cached_document(article.id)?
            .context("Cache")?
            .body
            .is_empty()
    );
    lume::offline::release(&store, &images)?;
    assert!(store.offline_state()?.is_none());
    assert_eq!(store.article(article.id)?.id, article.id);
    Ok(())
}

#[test]
fn cancelled_offline_capture_is_resumable_and_does_not_claim_completion()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    store.set_queued(article.id, true)?;
    let state =
        lume::offline::capture(&store, View::Queue, OfflineLimits::default())?;

    let result = lume::offline::prepare(
        &store,
        &directory.path().join("images"),
        state,
        &AtomicBool::new(true),
        |_| {},
    )?;

    assert!(result.cancelled);
    assert!(!result.running);
    assert_eq!(result.items[0].status, OfflineStatus::Pending);
    assert!(store.offline_state()?.context("Checkpoint")?.cancelled);
    assert!(
        lume::offline::capture(&store, View::All, OfflineLimits::default())
            .is_err()
    );
    Ok(())
}

fn candidate(id: i64, feed: i64, score: i64, minutes: u16) -> SessionItem {
    SessionItem {
        article: id,
        feed,
        url: format!("https://example.org/{id}"),
        canonical: format!("https://example.org/{id}"),
        title: format!("Article {id}"),
        minutes,
        score,
        topic: format!("Topic {feed}"),
    }
}

#[test]
fn session_planning_is_deterministic_diverse_and_budgeted() -> Result<()> {
    let mut duplicate = candidate(4, 3, 200, 3);
    duplicate.canonical = "https://example.org/1".into();
    let input = vec![
        candidate(1, 1, 300, 6),
        candidate(2, 1, 290, 4),
        candidate(3, 2, 280, 4),
        duplicate,
    ];

    let plan = lume::recommendation::session::plan(input.clone(), 10)?;
    let again = lume::recommendation::session::plan(input, 10)?;

    assert_eq!(
        plan.items
            .iter()
            .map(|item| item.article)
            .collect::<Vec<_>>(),
        [1, 3]
    );
    assert_eq!(
        serde_json::to_string(&plan)?,
        serde_json::to_string(&again)?
    );
    assert_eq!(plan.items.iter().map(|item| item.minutes).sum::<u16>(), 10);
    assert!(lume::recommendation::session::plan(Vec::new(), 0).is_err());
    Ok(())
}

#[test]
fn every_personal_context_protects_its_full_text_from_cleanup() -> Result<()> {
    for protection in [
        "favorite",
        "queue",
        "note",
        "highlight",
        "folder",
        "session",
    ] {
        let (directory, store, _) = common::seeded_store()?;
        let article = store.library()?.articles.remove(0);
        cache(&store, article.id, &article.url, "Protected full content.")?;
        protect_cache(&store, &article, protection)?;

        lume::storage::clean(
            &store,
            &directory.path().join("images"),
            lume::storage::Cleanup::All,
            &AtomicBool::new(false),
        )?;

        assert_eq!(
            store.cached_document(article.id)?.context("Cache")?.body,
            "Protected full content.",
            "Protection {protection}"
        );
    }
    Ok(())
}

#[test]
fn saved_pages_remain_outside_declarative_subscriptions() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_web_document(&page("https://example.org/saved"))?;
    let path = directory.path().join("config.toml");

    lume::config::Session::open(&store, &path)?.flush()?;

    let document = lume::config::decode(&std::fs::read_to_string(path)?)?;
    assert!(document.feeds.is_empty());
    assert_eq!(document.folders.len(), 1);
    assert_eq!(document.folders[0].name, "Saved pages");
    Ok(())
}

#[test]
fn saving_an_existing_feed_page_retains_full_text_without_overwriting_sources()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    let feed = store.body(article.id)?;
    let document = page(&article.url);

    assert_eq!(store.save_web_document(&document)?, article.id);

    assert_eq!(store.body(article.id)?, feed);
    assert_eq!(
        store
            .cached_document(article.id)?
            .context("Full cache")?
            .body,
        document.body
    );
    let mut later = document.clone();
    later.body = "A later page must not replace an anchored source.".into();
    store.save_web_document(&later)?;
    assert_eq!(
        store
            .cached_document(article.id)?
            .context("Full cache")?
            .body,
        document.body
    );
    assert_eq!(store.feeds()?.len(), 1);
    Ok(())
}

fn protect_cache(
    store: &Store,
    article: &lume::model::Article,
    protection: &str,
) -> Result<()> {
    match protection {
        "favorite" => store.set_starred(article.id, true)?,
        "queue" => store.set_queued(article.id, true)?,
        "note" => {
            store.save_comment(article.id, None, "Private note")?;
        }
        "highlight" => {
            store.save_passage(
                article.id,
                &lume::model::Passage {
                    quote: "Protected".into(),
                    anchor: 0,
                    source: lume::model::ContentSource::Full,
                    revision: 1,
                },
                "",
            )?;
        }
        "folder" => {
            let folder = store.create_collection("Protected")?;
            store.add_member(folder, article.id)?;
        }
        _ => {
            let mut item = candidate(article.id, article.feed_id, 10, 1);
            item.url = article.url.clone();
            store.save_reading_session(&lume::model::ReadingSession {
                budget: 10,
                items: vec![item],
                position: 0,
            })?;
        }
    }
    Ok(())
}

#[test]
fn orphan_image_cleanup_preserves_references_pinned_bytes_and_foreign_files()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let images = directory.path().join("images");
    let retained = "https://example.org/retained.svg";
    let orphan = "https://example.org/orphan.svg";
    let pinned = "https://example.org/pinned.svg";
    let mut document = page("https://example.org/images");
    document
        .body
        .push_str(&format!("\n\n![Diagram]({retained})\n"));
    store.save_web_document(&document)?;
    let svg = b"<svg xmlns='http://www.w3.org/2000/svg' width='1' height='1'/>";
    lume::media::cache::write(&images, retained, svg)?;
    lume::media::cache::write(&images, orphan, svg)?;
    lume::media::cache::write(&images.join("offline"), pinned, svg)?;
    std::fs::write(images.join("unrelated.txt"), "Foreign file")?;

    lume::storage::clean(
        &store,
        &images,
        lume::storage::Cleanup::Images,
        &AtomicBool::new(false),
    )?;

    assert_eq!(
        lume::media::cache::read(&images, retained)?.context("Referenced")?,
        svg
    );
    assert!(lume::media::cache::read(&images, orphan)?.is_none());
    assert_eq!(
        lume::media::cache::read(&images, pinned)?.context("Pinned")?,
        svg
    );
    assert_eq!(
        std::fs::read_to_string(images.join("unrelated.txt"))?,
        "Foreign file"
    );
    Ok(())
}

#[test]
fn a_recoverable_full_source_draft_protects_its_cache_revision() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    cache(&store, article.id, &article.url, "Protected full content.")?;
    store.save_note_draft(&lume::model::NoteDraft {
        target: lume::model::DraftTarget::Passage {
            article: article.id,
            passage: lume::model::Passage {
                quote: "Protected".into(),
                anchor: 0,
                source: lume::model::ContentSource::Full,
                revision: 1,
            },
        },
        body: "Unfinished private reasoning".into(),
        baseline: String::new(),
        initial_kind: None,
    })?;

    lume::storage::clean(
        &store,
        &directory.path().join("images"),
        lume::storage::Cleanup::All,
        &AtomicBool::new(false),
    )?;

    let cache = store.cached_document(article.id)?.context("Cache")?;
    assert_eq!(cache.revision, 1);
    assert_eq!(cache.body, "Protected full content.");
    assert_eq!(store.note_draft_count()?, 1);
    Ok(())
}
