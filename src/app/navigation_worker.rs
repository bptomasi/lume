//! Replaceable navigation reads have one connection and generation-checked results.

use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    latest::Latest,
    model::{ArticlePage, ArticleQuery},
    store::Store,
};

use super::{App, Mode, reading_data::ReadingData};

pub(super) enum Request {
    Page {
        query: ArticleQuery,
        selected: usize,
    },
    Article {
        id: i64,
        feed: i64,
    },
}

pub(super) struct Completed {
    page: Option<(usize, ArticlePage, usize)>,
    article: Option<i64>,
    reading: Option<ReadingData>,
}

pub(super) struct NavigationWorker {
    worker: Latest<Request, Completed>,
    ready: Option<Result<Completed>>,
}

impl NavigationWorker {
    fn start(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        Ok(Self {
            worker: Latest::start("lume-navigation", move |request| {
                read(&store, request)
            })?,
            ready: None,
        })
    }

    pub(super) fn cancel(&mut self) {
        self.worker.cancel();
        self.ready = None;
    }

    pub(super) fn submit(&mut self, request: Request) {
        self.ready = None;
        self.worker.submit(request);
    }
}

fn read(store: &Store, request: Request) -> Result<Completed> {
    let (page, article) = match request {
        Request::Page {
            mut query,
            selected,
        } => {
            let mut page = store.query_articles(&query)?;
            if page.articles.is_empty() && page.total > 0 {
                query.offset = (page.total - 1) / query.limit * query.limit;
                page = store.query_articles(&query)?;
            }
            let selected = selected.min(page.articles.len().saturating_sub(1));
            let article = page
                .articles
                .get(selected)
                .map(|article| (article.id, article.feed_id));
            (Some((query.offset, page, selected)), article)
        }
        Request::Article { id, feed } => (None, Some((id, feed))),
    };
    let reading = article
        .map(|(id, feed)| ReadingData::load(store, id, feed))
        .transpose()?;
    Ok(Completed {
        page,
        article: article.map(|(id, _)| id),
        reading,
    })
}

impl App {
    /// Enable bounded navigation reads and Markdown layout for interactive use.
    ///
    /// # Errors
    /// Returns database-opening or worker thread creation failures.
    pub fn enable_background_navigation(&mut self, path: &Path) -> Result<()> {
        let navigation = NavigationWorker::start(path)?;
        let session = super::session_writer::SessionWriter::start(path)?;
        self.renderer.enable_background_layout()?;
        self.wrap_reader();
        self.navigation_worker = Some(navigation);
        self.session_writer = Some(session);
        self.note_tasks = Some(super::note_tasks::NoteTasks::start(path)?);
        self.note_catalog =
            Some(super::note_catalog::NoteCatalog::start(path)?);
        Ok(())
    }

    pub(super) fn defer_navigation(&mut self, request: Request) {
        if let Some(worker) = &mut self.navigation_worker {
            worker.submit(request);
        }
        self.navigation_loading = true;
        self.clear_current();
        self.status =
            "Loading selection… · navigation remains available".into();
    }

    /// Whether the session worker still has unconfirmed durable writes.
    pub fn session_pending(&self) -> bool {
        self.session_writer
            .as_ref()
            .is_some_and(|writer| writer.pending())
    }

    /// Whether a failed admitted write still needs the user's attention.
    pub fn session_failed(&self) -> bool {
        self.session_writer
            .as_ref()
            .is_some_and(|writer| writer.failed())
    }

    /// Wait at an explicit shutdown boundary for all admitted writes.
    ///
    /// # Errors
    /// Reports write failures, disconnected workers and a five-second timeout.
    pub fn flush_session(&self) -> Result<()> {
        if let Some(writer) = &self.session_writer {
            writer.flush()?;
        }
        Ok(())
    }

    /// Apply only the newest admitted read, outside drafts and selections.
    ///
    /// # Errors
    /// Reports worker/storage failures while leaving the selected identity intact.
    pub fn poll_navigation(&mut self) -> Result<bool> {
        if let Some(error) = self
            .session_writer
            .as_ref()
            .and_then(|writer| writer.poll_error())
        {
            self.progress_memory.borrow_mut().clear();
            anyhow::bail!("Background save failed: {error}");
        }
        let Some(worker) = &mut self.navigation_worker else {
            return Ok(false);
        };
        if let Some(result) = worker.worker.poll() {
            worker.ready = Some(result);
        }
        if !matches!(self.mode, Mode::Normal) || self.selection.is_some() {
            return Ok(false);
        }
        let Some(result) = worker.ready.take() else {
            return Ok(false);
        };
        self.navigation_loading = false;
        let result = result.context("Navigation failed")?;
        if let Some((offset, page, selected)) = result.page {
            self.page_offset = offset;
            self.selected = selected;
            self.page_total = page.total;
            self.library.articles = page.articles;
            self.visible = (0..self.library.articles.len()).collect();
        }
        if self.current().map(|article| article.id) == result.article
            && let Some(reading) = result.reading
        {
            self.install_reading_data(reading)?;
        }
        self.status = "Selection loaded".into();
        Ok(true)
    }
}
