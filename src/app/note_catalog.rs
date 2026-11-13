//! Annotation-derived counts and filtered pages refresh away from input.
//!
//! A result installs only into the captured destination/query. The selected
//! article remains retained during a personal edit; navigating requests a fresh
//! page normally. A metadata refresh never reloads the current source body.

use super::{App, Mode};
use crate::{
    latest::Latest,
    model::{ArticlePage, ArticleQuery, View},
    store::Store,
};
use anyhow::Result;

struct Request {
    views: Vec<View>,
    query: Option<ArticleQuery>,
}

struct Snapshot {
    counts: Vec<(View, usize)>,
    page: Option<(ArticleQuery, ArticlePage)>,
}

pub(super) struct NoteCatalog {
    worker: Latest<Request, Snapshot>,
    ready: Option<Snapshot>,
    revision: u64,
}

impl NoteCatalog {
    pub(super) fn start(path: &std::path::Path) -> Result<Self> {
        let store = Store::open(path)?;
        Ok(Self {
            worker: Latest::start_cancellable(
                "lume-note-catalog",
                move |request: Request, cancel| {
                    store.read_cancellable(cancel, |store| {
                        let values = store.view_counts(&request.views)?;
                        let counts =
                            request.views.into_iter().zip(values).collect();
                        let page = request
                            .query
                            .map(|query| {
                                let page = store.query_articles(&query)?;
                                Ok::<_, anyhow::Error>((query, page))
                            })
                            .transpose()?;
                        Ok(Snapshot { counts, page })
                    })
                },
            )?,
            ready: None,
            revision: 0,
        })
    }
}

impl App {
    pub(super) fn refresh_note_catalog(&mut self) {
        let affected = super::navigation::sensitive(
            self.nav.view(),
            &self.nav.library().smart_lists,
            crate::search::Changes::NOTES,
        )
        .unwrap_or(true)
            || crate::search::parse(self.nav.search())
                .map(|query| query.depends_on(crate::search::Changes::NOTES))
                .unwrap_or(true);
        let query = affected.then(|| ArticleQuery {
            view: self.nav.view().clone(),
            text: self.nav.search().clone(),
            offset: self.nav.page_offset(),
            limit: self.nav.page_size().clamp(1, 500),
            retained: self.current().map(|article| article.id),
        });
        if let Some(catalog) = &mut self.note_catalog {
            catalog.ready = None;
            catalog.revision = self.nav.library_revision();
            catalog.worker.submit(Request {
                views: self
                    .nav
                    .navigation_sections()
                    .catalog()
                    .iter()
                    .filter(|row| {
                        super::navigation::sensitive(
                            &row.view,
                            &self.nav.library().smart_lists,
                            crate::search::Changes::NOTES,
                        )
                        .unwrap_or(true)
                    })
                    .map(|row| row.view.clone())
                    .collect(),
                query,
            });
        }
    }

    /// Install ready annotation-derived metadata without replacing a draft.
    ///
    /// # Errors
    /// Returns count/query errors; cached source and notes remain available.
    pub fn poll_note_catalog(&mut self) -> Result<bool> {
        let Some(catalog) = &mut self.note_catalog else {
            return Ok(false);
        };
        if let Some(result) = catalog.worker.poll() {
            if catalog.revision != self.nav.library_revision() {
                self.refresh_note_catalog();
                return Ok(false);
            }
            catalog.ready = Some(result?);
        }
        if !matches!(self.mode, Mode::Normal) || self.reader.selecting() {
            return Ok(false);
        }
        let Some(snapshot) = catalog.ready.take() else {
            return Ok(false);
        };
        if catalog.revision != self.nav.library_revision() {
            self.refresh_note_catalog();
            return Ok(false);
        }
        let counts = snapshot
            .counts
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        let mut navigation = self.nav.navigation_sections().catalog().to_vec();
        for row in &mut navigation {
            if let Some(count) = counts.get(&row.view) {
                row.count = *count;
            }
        }
        self.install_navigation(navigation);
        if let Some((query, page)) = snapshot.page {
            self.install_metadata_page(query, page)?;
        }
        Ok(true)
    }

    /// Whether annotation-derived metadata is still being read.
    pub fn note_catalog_pending(&self) -> bool {
        self.note_catalog
            .as_ref()
            .is_some_and(|catalog| catalog.worker.pending())
    }
}
