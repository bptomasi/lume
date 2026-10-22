//! Explicit, resumable offline preparation with bounded network and disk work.
//!
//! A cross-process file lock serializes packages. Checkpoints survive process
//! interruption. Cancellation is checked between bounded HTTP requests; it
//! cannot interrupt DNS or an already running request.

mod images;
pub(crate) use images::addresses as image_addresses;

use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};

use crate::{
    model::{
        ArticleQuery, OfflineItem, OfflineLimits, OfflineState, OfflineStatus,
        View,
    },
    store::Store,
};

/// Capture one folder or Read next in its current reading order.
///
/// # Errors
/// Returns unsupported destinations, invalid limits or query errors.
pub fn capture(
    store: &Store,
    view: View,
    limits: OfflineLimits,
) -> Result<OfflineState> {
    limits.validate()?;
    ensure!(
        matches!(view, View::Queue | View::Collection(_)),
        "Prepare Read next or a personal folder"
    );
    let page = store.query_articles(&ArticleQuery {
        view: view.clone(),
        text: String::new(),
        offset: 0,
        limit: limits.articles,
        retained: None,
    })?;
    let items = page
        .articles
        .into_iter()
        .map(|article| OfflineItem {
            article: article.id,
            url: article.url,
            title: crate::content::label(&article.title, 200),
            status: OfflineStatus::Pending,
            bytes: 0,
            images: 0,
            error: String::new(),
        })
        .collect();
    Ok(OfflineState {
        name: match view {
            View::Collection(id) => format!("Folder {id}"),
            _ => "Read next".into(),
        },
        total: page.total,
        limits,
        items,
        running: false,
        cancelled: false,
    })
}

/// Prepare captured text and images, publishing durable per-article checkpoints.
///
/// # Errors
/// Returns lock, checkpoint or filesystem errors. Individual download failures
/// remain visible as partial/failed entries and do not discard other results.
pub fn prepare(
    store: &Store,
    directory: &Path,
    mut state: OfflineState,
    cancel: &AtomicBool,
    progress: impl Fn(&OfflineState),
) -> Result<OfflineState> {
    state.validate()?;
    let _lock = lock(directory)?;
    let client = crate::source::agent();
    state.running = true;
    state.cancelled = false;
    store.save_offline_state(&state)?;
    for index in 0..state.items.len() {
        if cancel.load(Ordering::Relaxed) {
            state.cancelled = true;
            break;
        }
        let used = state
            .items
            .iter()
            .enumerate()
            .filter(|(position, _)| *position != index)
            .map(|(_, item)| item.bytes)
            .sum::<u64>();
        let available = state.limits.bytes.saturating_sub(used);
        prepare_item(
            store,
            directory,
            &client,
            &mut state.items[index],
            available,
            cancel,
        );
        store.save_offline_state(&state)?;
        progress(&state);
    }
    state.running = false;
    state.cancelled |= cancel.load(Ordering::Relaxed);
    store.save_offline_state(&state)?;
    Ok(state)
}

fn prepare_item(
    store: &Store,
    directory: &Path,
    client: &ureq::Agent,
    item: &mut OfflineItem,
    available: u64,
    cancel: &AtomicBool,
) {
    item.bytes = 0;
    item.images = 0;
    item.error.clear();
    item.status = OfflineStatus::Failed;
    let result = prepare_text(store, client, item, available, cancel).and_then(
        |(body, complete)| {
            item.bytes = body.len() as u64;
            item.status = OfflineStatus::Partial;
            let mut image_progress = images::Progress::default();
            let images = images::prepare(
                directory,
                client,
                &body,
                available - item.bytes,
                cancel,
                &mut image_progress,
            );
            item.bytes += image_progress.bytes;
            item.images = image_progress.count;
            images?;
            if complete {
                item.status = OfflineStatus::Ready;
            }
            Ok(())
        },
    );
    if let Err(error) = result {
        item.error = crate::content::label(&format!("{error:#}"), 400);
    }
}

fn prepare_text(
    store: &Store,
    client: &ureq::Agent,
    item: &mut OfflineItem,
    available: u64,
    cancel: &AtomicBool,
) -> Result<(String, bool)> {
    let article = store.article(item.article)?;
    ensure!(
        article.url == item.url && !store.is_deleted(item.article)?,
        "Offline target changed or was deleted"
    );
    if let Some(document) = store.cached_document(item.article)?
        && document.url == item.url
        && !document.body.is_empty()
        && crate::source::article::rules::rejection(&document.body).is_none()
    {
        ensure!(
            document.body.len() as u64 <= available,
            "Offline package byte limit reached"
        );
        return Ok((document.body, true));
    }
    let selector = store.article_selector(article.feed_id)?;
    let downloaded = crate::source::article::fetch_with_selector(
        client, &item.url, &selector,
    );
    ensure!(!cancel.load(Ordering::Relaxed), "Preparation cancelled");
    match downloaded {
        Ok(document) => {
            ensure!(
                document.body.len() as u64 <= available,
                "Offline package byte limit reached"
            );
            let target = crate::model::ArticleTarget {
                article: item.article,
                url: item.url.clone(),
            };
            ensure!(
                store.cache_document(&target, &document)?,
                "Offline target changed during download"
            );
            Ok((document.body, true))
        }
        Err(error) => feed_fallback(store, item, available, &error),
    }
}

fn feed_fallback(
    store: &Store,
    item: &mut OfflineItem,
    available: u64,
    error: &anyhow::Error,
) -> Result<(String, bool)> {
    item.error =
        crate::content::label(&format!("Feed fallback: {error:#}"), 400);
    ensure!(
        store.article(item.article)?.url == item.url
            && !store.is_deleted(item.article)?,
        "Offline target changed during download"
    );
    let body = store.body(item.article)?;
    ensure!(
        crate::content::body_kind(&body) == crate::content::BodyKind::Text,
        "No readable feed or full text available"
    );
    ensure!(
        body.len() as u64 <= available,
        "Offline package byte limit reached"
    );
    Ok((body, false))
}

fn lock(directory: &Path) -> Result<std::fs::File> {
    std::fs::create_dir_all(directory)?;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(directory.join("offline.lock"))?;
    file.try_lock()
        .context("Another offline preparation is running")?;
    Ok(file)
}

/// Remove only owned pinned image files and the package checkpoint.
///
/// # Errors
/// Returns concurrent-preparation, filesystem or SQLite errors.
pub fn release(store: &Store, directory: &Path) -> Result<()> {
    let _lock = lock(directory)?;
    images::release(&directory.join("offline"))?;
    store.release_offline_state()
}
