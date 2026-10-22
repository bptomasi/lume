//! Replaceable navigation reads have one connection and generation-checked results.

use std::path::Path;

use anyhow::{Context, Result};

use crate::{
    latest::Latest,
    model::{ArticlePage, ArticleQuery},
    store::Store,
};

use super::{App, Mode, reading_data::ReadingData};

#[derive(Clone)]
pub(super) enum Request {
    Search {
        query: ArticleQuery,
        selected: usize,
    },
    Page {
        query: ArticleQuery,
        selected: usize,
    },
    Jump {
        query: ArticleQuery,
        article: i64,
    },
    Refresh {
        query: ArticleQuery,
        selected: usize,
        article: Option<i64>,
    },
    Article {
        id: i64,
        feed: i64,
    },
}

pub(super) enum Arrival {
    Reader(super::ReaderLocation),
    Synthesis(String),
}

pub(super) struct Completed {
    page: Option<(usize, ArticlePage, usize)>,
    article: Option<i64>,
    reading: Option<ReadingData>,
}

pub(super) struct NavigationWorker {
    worker: Latest<Request, Completed>,
    ready: Option<Result<Completed>>,
    revision: u64,
    preserve_identity: bool,
    request: Option<Request>,
    arrival: Option<Arrival>,
}

impl NavigationWorker {
    fn start(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        Ok(Self {
            worker: Latest::start("lume-navigation", move |request| {
                store.read_snapshot(|store| read(store, request))
            })?,
            ready: None,
            revision: 0,
            preserve_identity: false,
            request: None,
            arrival: None,
        })
    }

    pub(super) fn cancel(&mut self) {
        self.worker.cancel();
        self.ready = None;
        self.request = None;
        self.arrival = None;
    }

    pub(super) fn submit(&mut self, request: Request, revision: u64) {
        self.ready = None;
        self.revision = revision;
        self.preserve_identity = matches!(request, Request::Article { .. });
        self.request = Some(request.clone());
        self.arrival = None;
        self.worker.submit(request);
    }
}

fn read(store: &Store, request: Request) -> Result<Completed> {
    match request {
        Request::Search { query, selected } => {
            read_search(store, query, selected)
        }
        Request::Article { id, feed } => Ok(Completed {
            page: None,
            article: Some(id),
            reading: Some(ReadingData::load(store, id, feed)?),
        }),
        Request::Page { query, selected } => read_page(store, query, selected),
        Request::Jump { query, article } => read_jump(store, query, article),
        Request::Refresh {
            query,
            selected,
            article,
        } => {
            if let Some(id) = article
                && let Some((offset, page)) =
                    store.page_containing(&query, id)?
            {
                let selected = page
                    .articles
                    .iter()
                    .position(|item| item.id == id)
                    .unwrap_or(0);
                complete_page(store, offset, page, selected)
            } else {
                read_page(store, query, selected)
            }
        }
    }
}

fn read_jump(
    store: &Store,
    query: ArticleQuery,
    article: i64,
) -> Result<Completed> {
    let (offset, page) = store
        .page_containing(&query, article)?
        .context("Article no longer belongs to this destination")?;
    let selected = page
        .articles
        .iter()
        .position(|item| item.id == article)
        .context("Jump target missing from its page")?;
    complete_page(store, offset, page, selected)
}

// Preview metadata is published independently of source loading and reflow.
fn read_search(
    store: &Store,
    query: ArticleQuery,
    selected: usize,
) -> Result<Completed> {
    let (offset, page) = query_page(store, query)?;
    let selected = selected.min(page.articles.len().saturating_sub(1));
    Ok(Completed {
        article: page.articles.get(selected).map(|article| article.id),
        page: Some((offset, page, selected)),
        reading: None,
    })
}

fn read_page(
    store: &Store,
    query: ArticleQuery,
    selected: usize,
) -> Result<Completed> {
    let (offset, page) = query_page(store, query)?;
    complete_page(store, offset, page, selected)
}

fn query_page(
    store: &Store,
    mut query: ArticleQuery,
) -> Result<(usize, ArticlePage)> {
    let mut page = store.query_articles(&query)?;
    if page.articles.is_empty() && page.total > 0 {
        query.offset = (page.total - 1) / query.limit * query.limit;
        page = store.query_articles(&query)?;
    }
    Ok((query.offset, page))
}

fn complete_page(
    store: &Store,
    offset: usize,
    page: ArticlePage,
    selected: usize,
) -> Result<Completed> {
    let selected = selected.min(page.articles.len().saturating_sub(1));
    let target = page.articles.get(selected);
    Ok(Completed {
        reading: target
            .map(|item| ReadingData::load(store, item.id, item.feed_id))
            .transpose()?,
        article: target.map(|item| item.id),
        page: Some((offset, page, selected)),
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
        self.catalog_worker =
            Some(super::catalog_worker::CatalogWorker::start(path)?);
        self.session_writer = Some(session);
        self.recommendations =
            Some(super::recommendations::Recommendations::start(path)?);
        self.personal_writes =
            Some(super::personal_tasks::PersonalWrites::start(path)?);
        self.note_catalog =
            Some(super::note_catalog::NoteCatalog::start(path)?);
        Ok(())
    }

    /// Start disposable startup projections after the first terminal frame.
    pub fn load_workspace(&mut self) {
        self.request_catalog(super::catalog_worker::Scope::Counts);
        self.defer_navigation(Request::Refresh {
            query: self.article_query(),
            selected: self.selected,
            article: self.workspace.article,
        });
    }

    pub(super) fn defer_navigation(&mut self, request: Request) {
        self.performance.begin(super::Operation::Article);
        self.cancel_reader_pointer();
        self.note_marks.hover = None;
        self.selection = None;
        self.projecting_navigation = false;
        if let Some(worker) = &mut self.navigation_worker {
            worker.submit(request, self.library_revision);
        }
        self.navigation_loading = true;
        self.status =
            "Loading selection… · navigation remains available".into();
    }

    pub(super) fn refresh_catalog_page(&mut self) {
        self.projecting_navigation = true;
        self.note_marks.hover = None;
        let request = Request::Refresh {
            query: self.article_query(),
            selected: self.selected,
            article: self.current().map(|article| article.id),
        };
        if let Some(worker) = &mut self.navigation_worker {
            worker.submit(request, self.library_revision);
        }
    }

    fn refresh_stale_navigation(&mut self) {
        if let Some(worker) = &mut self.navigation_worker
            && let Some(request @ Request::Jump { .. }) = worker.request.clone()
        {
            let arrival = worker.arrival.take();
            worker.submit(request, self.library_revision);
            worker.arrival = arrival;
            return;
        }

        let preserve = self
            .navigation_worker
            .as_ref()
            .is_some_and(|worker| worker.preserve_identity);
        if preserve
            && !self.article_edits.reconcile_required
            && let Some(article) = self.current()
            && self.retained_article == Some(article.id)
        {
            self.defer_navigation(Request::Article {
                id: article.id,
                feed: article.feed_id,
            });
        } else {
            self.refresh_catalog_page();
        }
    }

    /// Whether a catalog or page refresh still awaits safe delivery.
    pub fn catalog_pending(&self) -> bool {
        self.catalog_loading
            || self.navigation_worker.as_ref().is_some_and(|worker| {
                worker.worker.pending() || worker.ready.is_some()
            })
    }

    pub(super) fn session_article(&self) -> Option<i64> {
        if !self.navigation_loading {
            return self.current().map(|article| article.id);
        }
        match self
            .navigation_worker
            .as_ref()
            .and_then(|worker| worker.request.as_ref())
        {
            Some(Request::Jump { article, .. }) => Some(*article),
            Some(Request::Article { id, .. }) => Some(*id),
            Some(Request::Refresh {
                article: Some(article),
                ..
            }) => Some(*article),
            _ => self
                .current()
                .map(|article| article.id)
                .or(self.workspace.article),
        }
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

    /// Deliver the latest read during navigation or disposable query editing.
    /// Source-bound drafts and selections retain their captured article.
    ///
    /// # Errors
    /// Reports worker/storage failures while leaving the selected identity intact.
    pub fn poll_navigation(&mut self) -> Result<bool> {
        if let Some(error) = self
            .session_writer
            .as_ref()
            .and_then(|writer| writer.poll_error())
        {
            self.session_problem = Some(error.clone());
            if self.session_failed() {
                self.progress_memory.borrow_mut().clear();
                anyhow::bail!(
                    "Background save failed: {error} · :save-status details · :w retry"
                );
            }
            self.status =
                "Earlier session save recovered · :save-status details".into();
        }
        let catalog_changed =
            self.poll_configuration()? | self.poll_catalog()?;
        let delivery = self.navigation_delivery_allowed();
        let Some(worker) = &mut self.navigation_worker else {
            return Ok(catalog_changed);
        };
        if let Some(result) = worker.worker.poll() {
            worker.ready = Some(result);
        }
        if !delivery {
            return Ok(catalog_changed);
        }
        let Some(result) = worker.ready.take() else {
            return Ok(catalog_changed);
        };
        if worker.revision != self.library_revision {
            self.refresh_stale_navigation();
            return Ok(catalog_changed);
        }
        Ok(self.publish_navigation(result)? || catalog_changed)
    }

    fn navigation_delivery_allowed(&self) -> bool {
        if self.selection.is_some() {
            return false;
        }
        if matches!(self.mode, Mode::Normal) {
            return true;
        }
        // Query editors capture no article mutation. They receive live
        // results; source-bound drafts and explicit jump arrivals still wait.
        matches!(
            &self.mode,
            Mode::Input(input) if matches!(
                input.kind,
                super::InputKind::Search | super::InputKind::LibrarySearch
            )
        ) && self
            .navigation_worker
            .as_ref()
            .is_none_or(|worker| worker.arrival.is_none())
    }

    fn publish_navigation(
        &mut self,
        result: Result<Completed>,
    ) -> Result<bool> {
        if let Ok(completed) = &result
            && let Some(reading) = &completed.reading
        {
            let article = completed
                .page
                .as_ref()
                .and_then(|(_, page, selected)| page.articles.get(*selected))
                .or_else(|| self.current());
            let url = article.map_or("", |article| article.url.as_str());
            let full = self.workspace.content_source
                == crate::model::ContentSource::Full;
            let body = reading.source_body(full, url);
            if !self.renderer.prepare_source(body, self.reader_width) {
                self.projecting_navigation = true;
                if let Some(worker) = &mut self.navigation_worker {
                    worker.ready = Some(result);
                }
                return Ok(false);
            }
        }
        let navigating = self.navigation_loading;
        self.navigation_loading = false;
        self.projecting_navigation = false;
        self.apply_navigation(result.context("Navigation failed")?)?;
        if navigating {
            self.status = "Selection loaded".into();
        }
        if let Some(arrival) = self
            .navigation_worker
            .as_mut()
            .and_then(|worker| worker.arrival.take())
        {
            self.apply_navigation_arrival(arrival)?;
        }
        Ok(true)
    }

    pub(super) fn on_navigation_arrival(
        &mut self,
        arrival: Arrival,
    ) -> Result<()> {
        if let Some(worker) = &mut self.navigation_worker {
            worker.arrival = Some(arrival);
            Ok(())
        } else {
            self.apply_navigation_arrival(arrival)
        }
    }

    fn apply_navigation_arrival(&mut self, arrival: Arrival) -> Result<()> {
        match arrival {
            Arrival::Reader(location) => self.apply_reader_location(location),
            Arrival::Synthesis(text) => self.install_synthesis(text),
        }
    }

    pub(super) fn invalidate_reading_refresh(&mut self) {
        self.library_revision = self.library_revision.wrapping_add(1);
        if self.catalog_loading {
            self.request_catalog(super::catalog_worker::Scope::Counts);
        }
        if self.projecting_navigation && !self.navigation_loading {
            self.refresh_catalog_page();
        }
    }

    fn apply_navigation(&mut self, result: Completed) -> Result<()> {
        let snapshot = result.reading.as_ref().map(|_| self.capture_reader());
        if let Some((offset, page, selected)) = result.page {
            self.article_edits.reconcile_required = false;
            self.page_offset = offset;
            self.selected = selected;
            self.page_total = page.total;
            self.library.articles = page.articles;
            self.visible = (0..self.library.articles.len()).collect();
            self.overlay_article_edits();
        }
        if self.current().map(|article| article.id) == result.article {
            if let Some(reading) = result.reading {
                self.install_reading_data(reading)?;
                if let Some(snapshot) = snapshot {
                    self.restore_reader(snapshot);
                }
            } else if result.article.is_none() {
                self.clear_current();
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use anyhow::{Context, Result};

    #[test]
    fn an_obsolete_storage_error_is_replaced_after_a_personal_commit()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let store = crate::store::Store::open(&path)?;
        let mut app = App::new(store, false)?;
        app.enable_background_navigation(&path)?;
        app.navigation_worker
            .as_mut()
            .context("Navigation worker")?
            .ready = Some(Err(anyhow::anyhow!("obsolete page failure")));
        app.library_revision = 1;

        assert!(!app.poll_navigation()?);

        assert!(app.catalog_pending(), "Latest page was not requested");
        assert!(!app.status.contains("obsolete"));
        app.end_session()?;
        Ok(())
    }
}
