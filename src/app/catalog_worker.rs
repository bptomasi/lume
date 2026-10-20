//! Catalog counts have an independent bounded lane so they cannot queue ahead
//! of article navigation. Only the newest WAL snapshot reaches the workspace.

use std::path::Path;

use anyhow::{Context, Result};

use super::{App, Mode, NavItem};
use crate::{latest::Latest, model::Library, store::Store};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Counts,
    Library,
}

struct Catalog {
    library: Library,
    navigation: Vec<NavItem>,
}

pub(super) struct CatalogWorker {
    worker: Latest<i64, Catalog>,
    ready: Option<Result<Catalog>>,
    revision: u64,
    scope: Scope,
}

impl CatalogWorker {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        Ok(Self {
            worker: Latest::start("lume-catalog", move |last_visit| {
                store.read_snapshot(|store| {
                    let library = store.catalog()?;
                    let navigation = super::navigation::catalog_navigation(
                        store, &library, last_visit,
                    )?;
                    Ok(Catalog {
                        library,
                        navigation,
                    })
                })
            })?,
            ready: None,
            revision: 0,
            scope: Scope::Counts,
        })
    }
}

impl App {
    pub(super) fn request_catalog(&mut self, scope: Scope) {
        self.catalog_loading = true;
        if let Some(worker) = &mut self.catalog_worker {
            let pending = worker.worker.pending() || worker.ready.is_some();
            if !pending || scope == Scope::Library {
                worker.scope = scope;
            }
            worker.ready = None;
            worker.revision = self.library_revision;
            worker.worker.submit(self.workspace.last_visit);
        }
    }

    pub(super) fn poll_catalog(&mut self) -> Result<bool> {
        let Some(worker) = &mut self.catalog_worker else {
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
        self.catalog_loading = false;
        let scope = worker.scope;
        if worker.revision != self.library_revision {
            self.request_catalog(scope);
            return Ok(false);
        }
        self.publish_catalog(result.context("Catalog refresh failed")?, scope)?;
        Ok(true)
    }

    fn publish_catalog(
        &mut self,
        mut catalog: Catalog,
        scope: Scope,
    ) -> Result<()> {
        catalog.library.articles = std::mem::take(&mut self.library.articles);
        self.library = catalog.library;
        let view = self.view.clone();
        self.install_navigation(catalog.navigation);
        if !view.same_destination(&self.view) {
            self.page_offset = 0;
            self.selected = 0;
            self.retained_article = None;
            self.filter()?;
        } else if scope == Scope::Library && !self.navigation_loading {
            self.refresh_catalog_page();
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use anyhow::{Context, Result};

    #[test]
    fn an_obsolete_catalog_error_is_replaced_after_a_personal_commit()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let store = crate::store::Store::open(&path)?;
        let mut app = App::new(store, false)?;
        app.enable_background_navigation(&path)?;
        app.catalog_worker.as_mut().context("Catalog worker")?.ready =
            Some(Err(anyhow::anyhow!("obsolete count failure")));
        app.library_revision = 1;

        assert!(!app.poll_catalog()?);

        assert!(app.catalog_pending(), "Latest counts were not requested");
        assert!(!app.status.contains("obsolete"));
        app.end_session()?;
        Ok(())
    }
    #[test]
    fn a_counts_request_cannot_downgrade_a_pending_library_refresh()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let store = crate::store::Store::open(&path)?;
        let mut app = App::new(store, false)?;
        app.enable_background_navigation(&path)?;

        app.request_catalog(super::Scope::Library);
        app.request_catalog(super::Scope::Counts);

        assert!(
            app.catalog_worker.as_ref().context("Catalog worker")?.scope
                == super::Scope::Library
        );
        app.end_session()?;
        Ok(())
    }
}
