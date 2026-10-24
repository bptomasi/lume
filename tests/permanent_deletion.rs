//! Explicit deletion removes owned state atomically without reviving publishers.

mod common;

use anyhow::{Context, Result};
use lume::{
    model::{
        ArticleQuery, DraftTarget, IncomingArticle, NoteDraft, SmartFilter,
        View,
    },
    store::Store,
};

fn seed(
    store: &mut Store,
    feed: &lume::model::Feed,
    count: usize,
) -> Result<Vec<i64>> {
    let articles = (0..count)
        .map(|id| IncomingArticle {
            external_id: format!("purge:{id}"),
            title: format!("Permanent deletion {id}"),
            url: format!("https://journal.example/purge/{id}"),
            author: String::new(),
            published: 2_000_000_000 + id as i64,
            body: "Unique purgeable source content".into(),
            body_origin: lume::model::FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(feed, &feed.title, &articles)?;
    Ok(store
        .library()?
        .articles
        .iter()
        .map(|item| item.id)
        .collect())
}

#[test]
fn permanent_deletion_removes_notes_drafts_search_and_keeps_tombstones()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let ids = seed(&mut store, &feed, 1)?;
    let (target, other) = (ids[0], ids[1]);
    store.clear_history()?;
    store.set_starred(other, true)?;
    store.set_starred(target, true)?;
    let note = store.save_comment(target, None, "uniquepurgedannotation")?;
    store.save_collection("Keep collection", Some(target))?;
    store.save_note_draft(&NoteDraft {
        target: DraftTarget::Note {
            article: target,
            note: Some(note),
        },
        body: "Private draft".into(),
        baseline: String::new(),
        initial_kind: None,
    })?;
    store.set_deleted(target, true)?;
    assert!(store.purge_deleted(target)?);
    assert!(!store.purge_deleted(target)?);
    assert!(store.article(target).is_err());
    assert!(store.comments(target)?.is_empty());
    assert!(store.note_drafts()?.is_empty());
    assert!(store.library()?.collections[0].articles.is_empty());
    assert_eq!(search(&store, View::Trash, "uniquepurgedannotation")?, 0);
    let refresh =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    seed(&mut store, &feed, 1)?;
    store.ingest(&feed, &feed.title, &refresh.articles)?;
    assert_eq!(store.library()?.articles.len(), 1);
    drop(store);
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.view_count(&View::Trash)?, 0);
    assert!(reopened.article(target).is_err());
    Ok(())
}

#[test]
fn purging_membership_preserves_folder_creation_and_unrelated_flag_undo()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let ids = seed(&mut store, &feed, 1)?;
    let (target, other) = (ids[0], ids[1]);
    store.clear_history()?;
    store.set_starred(other, true)?;
    store.save_collection("Keep collection", Some(target))?;
    store.set_deleted(target, true)?;
    assert!(store.purge_deleted(target)?);
    assert!(store.undo()?);
    assert!(store.library()?.collections.is_empty());
    assert!(store.article(other)?.starred);
    assert!(store.undo()?);
    assert!(!store.article(other)?.starred);
    assert!(!store.undo()?);
    assert!(store.article(target).is_err());
    Ok(())
}

#[test]
fn trash_cleanup_rejects_restored_targets_and_keeps_live_articles() -> Result<()>
{
    let (_directory, mut store, feed) = common::seeded_store()?;
    let ids = seed(&mut store, &feed, 3)?;
    store.set_deleted(ids[0], true)?;
    store.set_deleted(ids[1], true)?;
    store.set_deleted(ids[1], false)?;
    assert!(store.purge_deleted(ids[1]).is_err());
    assert_eq!(store.clean_trash()?, 1);
    assert_eq!(store.clean_trash()?, 0);
    assert_eq!(store.view_count(&View::Trash)?, 0);
    assert_eq!(store.view_count(&View::All)?, 3);
    assert!(store.article(ids[1]).is_ok());
    Ok(())
}

#[test]
fn failed_cleanup_rolls_back_notes_history_and_tombstones() -> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let target = store.library()?.articles[0].id;
    store.save_comment(target, None, "Rollback preserves my note")?;
    store.set_deleted(target, true)?;
    let connection =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    connection.execute_batch(include_str!(
        "../src/store/sql/deletion/test_reject_purge.sql"
    ))?;
    assert!(store.clean_trash().is_err());
    assert!(store.is_deleted(target)?);
    assert_eq!(store.comments(target)?.len(), 1);
    assert!(store.undo()?);
    assert!(!store.is_deleted(target)?);
    assert!(store.unsubscribe(feed.id).is_ok());
    assert_eq!(store.view_count(&View::All)?, 1);
    Ok(())
}

#[test]
fn removing_a_feed_preserves_personal_saves_but_not_smart_list_matches()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let ids = seed(&mut store, &feed, 7)?;
    store.set_starred(ids[0], true)?;
    store.set_queued(ids[1], true)?;
    store.save_collection("Personal", Some(ids[2]))?;
    store.save_comment(ids[3], None, "Keep my source")?;
    store.save_note_draft(&NoteDraft {
        target: DraftTarget::Note {
            article: ids[4],
            note: None,
        },
        body: "Keep my draft".into(),
        baseline: String::new(),
        initial_kind: None,
    })?;
    store
        .set_tags(lume::model::TagTarget::Article(ids[5]), &["keep".into()])?;
    store.save_smart_list("Everything", &SmartFilter::default())?;
    let smart = store.smart_lists()?.remove(0).id;
    store.set_deleted(ids[6], true)?;
    store.unsubscribe(feed.id)?;
    assert!(store.feeds()?.is_empty());
    for id in &ids[..6] {
        assert!(store.article(*id).is_ok());
    }
    for id in &ids[6..] {
        assert!(store.article(*id).is_err());
    }
    assert_eq!(store.view_count(&View::Smart(smart))?, 6);
    assert_eq!(store.view_count(&View::Trash)?, 0);
    assert!(store.undo()?); // Unrelated tag edit survives cleanup.
    assert_eq!(store.view_count(&View::All)?, 6);
    Ok(())
}

fn search(store: &Store, view: View, text: &str) -> Result<usize> {
    Ok(store
        .query_articles(&ArticleQuery {
            view,
            text: text.into(),
            offset: 0,
            limit: 10,
            retained: None,
        })
        .context("Search after purge")?
        .total)
}

#[test]
fn purge_updates_offline_session_and_workspace_targets_without_dangling_ids()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let ids = seed(&mut store, &feed, 3)?;
    save_session(&store, &ids, feed.id)?;
    save_package(&store, &ids)?;
    let mut workspace = store.workspace()?;
    workspace.article = Some(ids[1]);
    store.save_workspace(&workspace)?;
    for id in &ids[..2] {
        store.set_deleted(*id, true)?;
    }
    assert_eq!(store.clean_trash()?, 2);
    let session = store.reading_session()?.context("Reading session")?;
    assert_eq!(session.position, 0);
    assert_eq!(
        session
            .items
            .iter()
            .map(|item| item.article)
            .collect::<Vec<_>>(),
        ids[2..]
    );
    let package = store.offline_state()?.context("Offline package")?;
    assert_eq!(
        package
            .items
            .iter()
            .map(|item| item.article)
            .collect::<Vec<_>>(),
        ids[2..]
    );
    assert_eq!(store.workspace()?.article, None);
    Ok(())
}

#[test]
fn permanent_cleanup_can_empty_every_saved_checkpoint() -> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let ids = seed(&mut store, &feed, 3)?;
    save_session(&store, &ids, feed.id)?;
    save_package(&store, &ids)?;
    for id in &ids {
        store.set_deleted(*id, true)?;
    }
    store.clean_trash()?;
    assert!(
        store
            .reading_session()?
            .context("Empty session")?
            .items
            .is_empty()
    );
    assert!(
        store
            .offline_state()?
            .context("Empty package")?
            .items
            .is_empty()
    );
    Ok(())
}

fn save_session(store: &Store, ids: &[i64], feed: i64) -> Result<()> {
    let items = ids
        .iter()
        .map(|id| lume::model::SessionItem {
            article: *id,
            feed,
            url: format!("https://journal.example/{id}"),
            canonical: String::new(),
            title: format!("Article {id}"),
            minutes: 1,
            score: 1,
            topic: String::new(),
        })
        .collect();
    store.save_reading_session(&lume::model::ReadingSession {
        budget: 10,
        items,
        position: 2,
    })
}

fn save_package(store: &Store, ids: &[i64]) -> Result<()> {
    let items = ids
        .iter()
        .map(|id| lume::model::OfflineItem {
            article: *id,
            url: format!("https://journal.example/{id}"),
            title: format!("Article {id}"),
            status: lume::model::OfflineStatus::Ready,
            bytes: 10,
            images: 0,
            error: String::new(),
        })
        .collect();
    store.save_offline_state(&lume::model::OfflineState {
        name: "Saved package".into(),
        total: ids.len(),
        limits: Default::default(),
        items,
        running: false,
        cancelled: false,
    })
}
