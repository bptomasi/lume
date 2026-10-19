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
}

impl NoteCatalog {
    pub(super) fn start(path: &std::path::Path) -> Result<Self> {
        let store = Store::open(path)?;
        Ok(Self {
            worker: Latest::start(
                "lume-note-catalog",
                move |request: Request| {
                    let counts = request
                        .views
                        .into_iter()
                        .map(|view| {
                            Ok((view.clone(), store.view_count(&view)?))
                        })
                        .collect::<Result<_>>()?;
                    let page = request
                        .query
                        .map(|query| {
                            let page = store.query_articles(&query)?;
                            Ok::<_, anyhow::Error>((query, page))
                        })
                        .transpose()?;
                    Ok(Snapshot { counts, page })
                },
            )?,
            ready: None,
        })
    }
}

impl App {
    pub(super) fn refresh_note_catalog(&mut self) {
        let query = (matches!(
            self.view,
            View::Smart(_) | View::Annotated | View::Recommended
        ) || !self.search.is_empty())
        .then(|| ArticleQuery {
            view: self.view.clone(),
            text: self.search.clone(),
            offset: self.page_offset,
            limit: self.page_size.clamp(1, 500),
            retained: self.current().map(|article| article.id),
        });
        if let Some(catalog) = &mut self.note_catalog {
            catalog.ready = None;
            catalog.worker.submit(Request {
                views: self
                    .navigation
                    .iter()
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
            catalog.ready = Some(result?);
        }
        if !matches!(self.mode, Mode::Normal) || self.selection.is_some() {
            return Ok(false);
        }
        let Some(snapshot) = catalog.ready.take() else {
            return Ok(false);
        };
        for (view, count) in snapshot.counts {
            if let Some(row) =
                self.navigation.iter_mut().find(|row| row.view == view)
            {
                row.count = count;
            }
        }
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
