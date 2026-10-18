//! Controlled I/O boundaries qualify queue limits, stale results and shutdown.

use std::{sync::mpsc, time::Duration};

use anyhow::{Context, Result};

use super::{ArticleTarget, ExtractedDocument, Store, Worker};

fn library() -> Result<(tempfile::TempDir, Store, ArticleTarget)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("worker.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let parsed = crate::source::parse(
        include_bytes!("../../../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let article = store.library()?.articles.remove(0);
    Ok((
        directory,
        store,
        ArticleTarget {
            article: article.id,
            url: article.url,
        },
    ))
}

fn document() -> ExtractedDocument {
    ExtractedDocument {
        final_url: "https://journal.example/full".into(),
        body: "A useful extracted worker result.".into(),
    }
}

#[test]
fn one_active_and_one_waiting_request_bound_the_queue_without_losing_results()
-> Result<()> {
    let (directory, store, target) = library()?;
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let worker =
        Worker::spawn(directory.path().join("worker.sqlite3"), move || {
            move |_url: &str, _selector: &str| {
                started.send(())?;
                releases.recv_timeout(Duration::from_secs(3))?;
                Ok(document())
            }
        })?;
    worker.fetch(target.clone())?;
    starts.recv_timeout(Duration::from_secs(3))?;
    worker.fetch(target.clone())?;
    assert!(worker.fetch(target.clone()).is_err());

    release.send(())?;
    let first = worker.results.recv_timeout(Duration::from_secs(3))?;
    assert!(first.error.is_none());
    starts.recv_timeout(Duration::from_secs(3))?;
    release.send(())?;
    let second = worker.results.recv_timeout(Duration::from_secs(3))?;
    assert_eq!(second.target, target);
    assert!(second.error.is_none());
    assert_eq!(
        store
            .cached_document(target.article)?
            .context("Cache missing")?
            .revision,
        1
    );
    Ok(())
}

#[test]
fn article_deleted_during_io_discards_the_downloaded_result() -> Result<()> {
    let (directory, store, target) = library()?;
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let worker =
        Worker::spawn(directory.path().join("worker.sqlite3"), move || {
            move |_url: &str, _selector: &str| {
                started.send(())?;
                releases.recv_timeout(Duration::from_secs(3))?;
                Ok(document())
            }
        })?;
    worker.fetch(target.clone())?;
    starts.recv_timeout(Duration::from_secs(3))?;
    store.set_deleted(target.article, true)?;

    release.send(())?;

    let completion = worker.results.recv_timeout(Duration::from_secs(3))?;
    assert!(
        completion
            .error
            .context("Stale download reported success")?
            .contains("discarded")
    );
    assert!(store.cached_document(target.article)?.is_none());
    assert!(store.body(target.article)?.contains("Keep **one** copy"));
    Ok(())
}

#[test]
fn a_fetch_failure_preserves_the_previous_cache_and_persists_its_reason()
-> Result<()> {
    let (directory, store, target) = library()?;
    store.cache_document(&target, &document())?;
    let worker =
        Worker::spawn(directory.path().join("worker.sqlite3"), || {
            |_url: &str, _selector: &str| {
                anyhow::bail!("Controlled publisher failure")
            }
        })?;

    worker.fetch(target.clone())?;

    let completion = worker.results.recv_timeout(Duration::from_secs(3))?;
    assert!(
        completion
            .error
            .context("Failure reported success")?
            .contains("Controlled publisher failure")
    );
    let cache = store
        .cached_document(target.article)?
        .context("Cache missing")?;
    assert_eq!(cache.body, "A useful extracted worker result.");
    assert_eq!(cache.error, "Controlled publisher failure");
    Ok(())
}

struct Stopped(mpsc::SyncSender<()>);

impl Drop for Stopped {
    fn drop(&mut self) {
        let _ = self.0.try_send(());
    }
}

#[test]
fn dropping_the_worker_during_io_prevents_a_later_cache_write() -> Result<()> {
    let (directory, store, target) = library()?;
    let (started, starts) = mpsc::channel();
    let (release, releases) = mpsc::channel();
    let (stopped, shutdown) = mpsc::sync_channel(1);
    let worker =
        Worker::spawn(directory.path().join("worker.sqlite3"), move || {
            let witness = Stopped(stopped);
            move |_url: &str, _selector: &str| {
                let _ = &witness;
                started.send(())?;
                releases.recv_timeout(Duration::from_secs(3))?;
                Ok(document())
            }
        })?;
    worker.fetch(target.clone())?;
    starts.recv_timeout(Duration::from_secs(3))?;

    drop(worker);
    release.send(())?;

    shutdown.recv_timeout(Duration::from_secs(3))?;
    assert!(store.cached_document(target.article)?.is_none());
    Ok(())
}
