//! One active extraction and one queued request, independent of feed collection.
//!
//! The worker owns HTTP and SQLite. Dropping it cancels between bounded jobs;
//! an in-flight HTTP request may finish. Cancellation is checked before its
//! cache write; a transaction already committing is allowed to complete.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};

use anyhow::{Context, Result, ensure};

use crate::{
    model::{ArticleTarget, ExtractedDocument, RulePreview, RuleRequest},
    store::Store,
};

/// Completion for the captured target; article text remains in SQLite.
#[derive(Debug)]
pub struct Completion {
    /// Article identity and URL that were requested.
    pub target: ArticleTarget,
    /// Failure or stale-target explanation, absent after a successful commit.
    pub error: Option<String>,
    /// Unsaved preview; neither the source rule nor cached text was changed.
    pub preview: Option<RulePreview>,
}

enum Request {
    Fetch(ArticleTarget),
    Preview(RuleRequest),
}

/// Dedicated article worker; no feed refresh can occupy its command queue.
pub struct Worker {
    requests: SyncSender<Request>,
    results: Receiver<Completion>,
    stopped: Arc<AtomicBool>,
}

impl Worker {
    /// Start one article worker with its own pooled HTTP agent.
    ///
    /// # Errors
    /// Returns an operating-system thread creation failure.
    pub fn start(path: PathBuf) -> Result<Self> {
        Self::spawn(path, || {
            let client = super::super::agent();
            move |address: &str, selector: &str| {
                super::fetch_with_selector(&client, address, selector)
            }
        })
    }

    fn spawn<F>(
        path: PathBuf,
        factory: impl FnOnce() -> F + Send + 'static,
    ) -> Result<Self>
    where
        F: Fn(&str, &str) -> Result<ExtractedDocument> + 'static,
    {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let cancellation = Arc::clone(&stopped);
        thread::Builder::new().name("lume-article".into()).spawn(
            move || {
                let fetch = factory();
                run(&path, receiver, sender, &cancellation, fetch);
            },
        )?;
        Ok(Self {
            requests,
            results,
            stopped,
        })
    }

    /// Submit a captured target without blocking navigation.
    ///
    /// # Errors
    /// Returns a busy/disconnected error; an unaccepted job is never started.
    pub fn fetch(&self, target: ArticleTarget) -> Result<()> {
        self.requests
            .try_send(Request::Fetch(target))
            .context("Article worker is busy or stopped")
    }

    /// Submit a rule preview without modifying the rule or successful cache.
    ///
    /// # Errors
    /// Returns invalid selectors or a busy/disconnected worker error.
    pub fn preview(&self, request: RuleRequest) -> Result<()> {
        super::rules::validate_selector(&request.selector)?;
        self.requests
            .try_send(Request::Preview(request))
            .context("Article worker is busy or stopped")
    }

    /// Receive a completion without waiting or loading an article body.
    pub fn poll(&self) -> Option<Completion> {
        self.results.try_recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

fn run(
    path: &Path,
    requests: Receiver<Request>,
    results: SyncSender<Completion>,
    stopped: &AtomicBool,
    fetch: impl Fn(&str, &str) -> Result<ExtractedDocument>,
) {
    while let Ok(request) = requests.recv() {
        if stopped.load(Ordering::Relaxed) {
            return;
        }
        let target = match &request {
            Request::Fetch(target) => target.clone(),
            Request::Preview(request) => request.target.clone(),
        };
        let result = match request {
            Request::Fetch(_) => {
                collect(path, &target, stopped, &fetch).map(|()| None)
            }
            Request::Preview(request) => {
                preview(path, request, &fetch).map(Some)
            }
        };
        if stopped.load(Ordering::Relaxed) {
            return;
        }
        let (preview, error) = match result {
            Ok(preview) => (preview, None),
            Err(error) => (None, Some(format!("{error:#}"))),
        };
        if results
            .send(Completion {
                target,
                error,
                preview,
            })
            .is_err()
        {
            return;
        }
    }
}

fn collect(
    path: &Path,
    target: &ArticleTarget,
    stopped: &AtomicBool,
    fetch: &impl Fn(&str, &str) -> Result<ExtractedDocument>,
) -> Result<()> {
    let store = Store::open(path)?;
    ensure!(
        !store.is_deleted(target.article)?
            && store.article(target.article)?.url == target.url,
        "Article changed or moved to Trash; request discarded"
    );
    let feed = store.article(target.article)?.feed_id;
    let selector = store.article_selector(feed)?;
    let result = fetch(&target.url, &selector);
    if stopped.load(Ordering::Relaxed) {
        return Ok(());
    }
    let result =
        result.and_then(|document| store.cache_document(target, &document));
    let cached = match result {
        Ok(cached) => cached,
        Err(error) => {
            store.document_failure(target, &format!("{error:#}"))?;
            return Err(error);
        }
    };
    ensure!(
        cached,
        "Article changed or moved to Trash; result discarded"
    );
    Ok(())
}

#[cfg(test)]
mod tests;

pub(super) fn collect_once(path: &Path, article: i64) -> Result<()> {
    let metadata = Store::open(path)?.article(article)?;
    let target = ArticleTarget {
        article,
        url: metadata.url,
    };
    let client = super::super::agent();
    collect(
        path,
        &target,
        &AtomicBool::new(false),
        &|address, selector| {
            super::fetch_with_selector(&client, address, selector)
        },
    )
}

fn preview(
    path: &Path,
    request: RuleRequest,
    fetch: &impl Fn(&str, &str) -> Result<ExtractedDocument>,
) -> Result<RulePreview> {
    let store = Store::open(path)?;
    let article = store.article(request.target.article)?;
    ensure!(
        !store.is_deleted(article.id)?
            && article.url == request.target.url
            && article.feed_id == request.feed,
        "Preview target changed or moved to Trash"
    );
    let document = fetch(&request.target.url, &request.selector)?;
    Ok(RulePreview {
        target: request.target,
        feed: request.feed,
        previous: request.previous,
        selector: request.selector,
        document,
    })
}
