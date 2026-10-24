//! Catalog counts have an independent bounded lane so they cannot queue ahead
//! of article navigation. Only the newest WAL snapshot reaches the workspace.

use std::path::Path;

use anyhow::{Context, Result};

use super::{App, Mode, NavItem};
use crate::{latest::Latest, model::Library, store::Store};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Scope {
    Changes(crate::search::Changes),
    Counts,
    Library,
}

impl Scope {
    fn covers(self, other: Self) -> bool {
        self.merge(other) == self
    }
    fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Library, _) | (_, Self::Library) => Self::Library,
            (Self::Counts, _) | (_, Self::Counts) => Self::Counts,
            (Self::Changes(a), Self::Changes(b)) => Self::Changes(a.union(b)),
        }
    }
}

struct Catalog {
    library: Library,
    navigation: Vec<NavItem>,
}

pub(super) struct CatalogWorker {
    worker: Latest<Request, Catalog>,
    ready: Option<Result<Catalog>>,
    revision: u64,
    scope: Scope,
}

struct Request {
    last_visit: i64,
    previous: Vec<NavItem>,
    scope: Scope,
}

fn read_catalog(store: &Store, request: Request) -> Result<Catalog> {
    let library = store.catalog()?;
    let navigation = if let Scope::Changes(changes) = request.scope {
        selective_counts(
            store,
            &library,
            request.last_visit,
            &request.previous,
            changes,
        )?
    } else {
        super::navigation::catalog_navigation(
            store,
            &library,
            request.last_visit,
        )?
    };
    Ok(Catalog {
        library,
        navigation,
    })
}

pub(super) fn read_counts(
    store: &Store,
    library: &Library,
    last_visit: i64,
    previous: &[NavItem],
) -> Result<Vec<NavItem>> {
    selective_counts(
        store,
        library,
        last_visit,
        previous,
        crate::search::Changes::READ,
    )
}

pub(super) fn selective_counts(
    store: &Store,
    library: &Library,
    last_visit: i64,
    previous: &[NavItem],
    changes: crate::search::Changes,
) -> Result<Vec<NavItem>> {
    let mut navigation =
        super::navigation::pending_navigation(library, last_visit);
    let previous = previous
        .iter()
        .map(|item| (&item.view, item.count))
        .collect::<std::collections::HashMap<_, _>>();
    let mut changed = Vec::new();
    for item in &mut navigation {
        let previous = previous.get(&item.view);
        if super::navigation::sensitive(
            &item.view,
            &library.smart_lists,
            changes,
        )? || previous.is_none()
        {
            changed.push(item.view.clone());
        } else if let Some(previous) = previous {
            item.count = *previous;
        }
    }
    let values = store.view_counts(&changed)?;
    let counts = changed
        .into_iter()
        .zip(values)
        .collect::<std::collections::HashMap<_, _>>();
    for item in &mut navigation {
        if let Some(count) = counts.get(&item.view) {
            item.count = *count;
        }
    }
    Ok(navigation)
}

impl CatalogWorker {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        Ok(Self {
            worker: Latest::start_cancellable(
                "lume-catalog",
                move |request, cancel| {
                    store.read_cancellable(cancel, |store| {
                        read_catalog(store, request)
                    })
                },
            )?,
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
            if pending
                && worker.revision == self.library_revision
                && worker.scope.covers(scope)
            {
                return;
            }
            worker.scope = if pending {
                scope.merge(worker.scope)
            } else {
                scope
            };
            worker.ready = None;
            worker.revision = self.library_revision;
            worker.worker.submit(Request {
                last_visit: self.workspace.last_visit,
                previous: if matches!(worker.scope, Scope::Changes(_)) {
                    self.navigation_sections.catalog().to_vec()
                } else {
                    Vec::new()
                },
                scope: worker.scope,
            });
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

    fn catalog_page_changed(&self, scope: Scope) -> Result<bool> {
        let Scope::Changes(changes) = scope else {
            return Ok(scope == Scope::Library);
        };
        if changes == crate::search::Changes::READ
            && self.retained_article.is_some()
        {
            return Ok(false);
        }
        Ok(super::navigation::sensitive(
            &self.view,
            &self.library.smart_lists,
            changes,
        )? || crate::search::parse(&self.search)?.depends_on(changes))
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
        } else if !self.navigation_loading
            && self.catalog_page_changed(scope)?
        {
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

#[cfg(test)]
mod invalidation_tests {
    use super::*;
    use anyhow::{Context, Result};

    #[test]
    fn read_refresh_does_not_evaluate_unrelated_full_text_lists() -> Result<()>
    {
        let (_directory, path, store) = library()?;
        let library = store.catalog()?;
        let previous =
            super::super::navigation::catalog_navigation(&store, &library, 0)?;
        let id = store.library()?.articles[0].id;
        store.set_unread(id, false)?;
        rusqlite::Connection::open(&path)?.execute_batch(include_str!(
            "../store/sql/test_drop_article_search.sql"
        ))?;
        let updated = read_counts(&store, &library, 0, &previous)?;
        let list = library
            .smart_lists
            .iter()
            .find(|list| list.name == "systems")
            .context("System list")?;
        assert_eq!(
            updated
                .iter()
                .find(|item| item.view == crate::model::View::Smart(list.id))
                .context("Count")?
                .count,
            1
        );
        assert_eq!(
            updated
                .iter()
                .find(|item| item.view == crate::model::View::Inbox)
                .context("Inbox")?
                .count,
            0
        );
        assert!(
            super::super::navigation::catalog_navigation(&store, &library, 0)
                .is_err()
        );
        Ok(())
    }
    #[test]
    fn read_updates_refresh_counts_hidden_inside_collapsed_sections()
    -> Result<()> {
        let (_directory, _path, store) = library()?;
        let mut app = App::new(store, false)?;
        app.workspace.collapsed_groups.push("SMART LISTS".into());
        app.project_navigation();
        let id = app.current().context("Article")?.id;
        app.store.set_unread(id, false)?;
        app.refresh_read_counts()?;
        let group = app
            .navigation
            .iter()
            .find(|item| {
                item.view == crate::model::View::Group("SMART LISTS".into())
            })
            .context("Collapsed group")?;
        assert_eq!(group.count, 1);
        app.workspace.collapsed_groups.clear();
        app.project_navigation();
        let unread = app
            .library
            .smart_lists
            .iter()
            .find(|list| list.name == "is:unread")
            .context("Unread list")?
            .id;
        assert_eq!(
            app.navigation
                .iter()
                .find(|item| item.view == crate::model::View::Smart(unread))
                .context("Expanded count")?
                .count,
            0
        );
        Ok(())
    }

    #[test]
    fn identical_pending_refreshes_keep_the_already_prepared_snapshot()
    -> Result<()> {
        let (_directory, path, store) = library()?;
        let mut app = App::new(store, false)?;
        app.enable_background_navigation(&path)?;
        let library = app.store.catalog()?;
        app.catalog_worker.as_mut().context("Worker")?.ready =
            Some(Ok(Catalog {
                library,
                navigation: app.navigation_sections.catalog().to_vec(),
            }));
        app.request_catalog(Scope::Counts);
        assert!(
            app.catalog_worker
                .as_ref()
                .context("Worker")?
                .ready
                .is_some()
        );
        app.request_catalog(Scope::Library);
        assert!(
            app.catalog_worker
                .as_ref()
                .context("Worker")?
                .ready
                .is_none()
        );
        app.end_session()
    }

    fn library() -> Result<(tempfile::TempDir, std::path::PathBuf, Store)> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let mut store = Store::open(&path)?;
        store.subscribe("https://journal.test/feed")?;
        let feed = store.feeds()?.remove(0);
        let parsed = crate::source::parse(
            include_bytes!("../../tests/fixtures/rss.xml"),
            &feed.url,
        )?;
        store.ingest(&feed, &parsed.title, &parsed.articles)?;
        for text in ["systems", "is:unread"] {
            store.save_smart_list(
                text,
                &crate::model::SmartFilter {
                    text: text.into(),
                    ..Default::default()
                },
            )?;
        }

        Ok((directory, path, store))
    }
}
