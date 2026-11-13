//! SQLite-backed navigation with bounded pages and no historical cutoff.
//!
//! Destination changes reuse catalog counts. Personal edits and collection
//! completion refresh those counts; moving a cursor does not mutate them.

use anyhow::Result;

use crate::model::{ArticlePage, ArticleQuery};

use super::{App, Focus, NavItem, View};

impl App {
    pub(super) fn pin_navigation_viewport(&mut self) {
        if let Some((first, skip)) = self.navigation_origin() {
            self.nav.pin_navigation(first, skip);
        }
    }

    fn pin_article_viewport(&mut self) {
        let areas = crate::presentation::geometry::workspace_areas(
            self.layout.screen(),
            self.layout.focus(),
            self.layout.workspace(),
        );
        if let Some(area) = areas.articles {
            self.nav
                .pin_articles(self.navigation_hits().article_origin(area));
        }
    }

    pub(super) fn filter_preview(&mut self) -> Result<()> {
        if self.navigation_worker.is_none() {
            return self.filter_now();
        }
        self.defer_navigation(super::navigation_worker::Request::Search {
            query: self.article_query(),
            selected: self.nav.selected(),
        });
        Ok(())
    }

    pub(super) fn commit_article_search(&mut self) -> Result<()> {
        if self.nav.navigation_loading() {
            self.filter()
        } else if self.reader_stale() {
            self.load_current_deferred()
        } else {
            Ok(())
        }
    }

    pub(crate) fn rebuild_navigation(&mut self) -> Result<()> {
        let items = catalog_navigation(
            &self.store,
            self.nav.library(),
            self.layout.workspace().last_visit,
        )?;
        self.install_navigation(items);
        Ok(())
    }

    pub(super) fn install_navigation(&mut self, items: Vec<NavItem>) {
        let editing = matches!(
            self.interaction.mode(),
            super::Mode::Input(input)
                if matches!(input.kind, super::InputKind::LibrarySearch)
        );
        self.nav
            .install_navigation(items, self.layout.workspace(), editing);
    }

    pub(super) fn project_navigation(&mut self) {
        self.nav.project_navigation(self.layout.workspace());
    }

    pub(crate) fn filter(&mut self) -> Result<()> {
        if matches!(self.nav.view(), View::Group(_)) {
            return self.clear_section_page();
        }
        let query = self.article_query();
        if self.navigation_worker.is_some() {
            self.defer_navigation(super::navigation_worker::Request::Page {
                query,
                selected: self.nav.selected(),
            });
            self.nav.clear_page();
            return Ok(());
        }
        self.filter_now()
    }

    pub(super) fn filter_now(&mut self) -> Result<()> {
        if matches!(self.nav.view(), View::Group(_)) {
            return self.clear_section_page();
        }
        let (offset, page) =
            self.store.article_window(&self.article_query())?;
        self.nav.install_offset_page(offset, page);
        self.load_current()
    }

    fn clear_section_page(&mut self) -> Result<()> {
        let clear = self.nav.navigation_loading()
            || self.nav.projecting_navigation()
            || !self.nav.library().articles.is_empty()
            || !self.reader.view().reader_text.is_empty();
        self.nav.clear_section();
        if clear {
            self.load_current()?;
        }
        Ok(())
    }

    pub(super) fn article_query(&self) -> ArticleQuery {
        self.nav.article_query()
    }

    pub(super) fn restore_page_selection(
        &mut self,
        article: i64,
    ) -> Result<()> {
        if !self
            .nav
            .library()
            .articles
            .iter()
            .any(|item| item.id == article)
        {
            let Some((offset, page)) =
                self.store.page_containing(&self.article_query(), article)?
            else {
                return Ok(());
            };
            self.nav.install_offset_page(offset, page);
            self.load_current()?;
        }
        if let Some(position) = self
            .nav
            .library()
            .articles
            .iter()
            .position(|item| item.id == article)
        {
            self.nav.select_article(position);
            self.load_current()?;
        }
        Ok(())
    }

    /// Switch destinations and release the retained inbox selection.
    ///
    /// # Errors
    /// Returns SQLite or content-loading errors.
    pub fn set_view(&mut self, view: View) -> Result<()> {
        self.pin_navigation_viewport();
        self.reveal_server(&view)?;
        self.close_notebook();
        self.save_reading_position()?;
        self.nav.select_destination(&view);
        self.filter()
    }

    pub(crate) fn move_selection(
        &mut self,
        forward: bool,
        distance: usize,
    ) -> Result<()> {
        match self.layout.focus() {
            Focus::Library => {
                let previous = self.nav.nav_selected();
                if let Some(next) = self.move_library_match(forward, distance)
                    && next != previous
                {
                    self.set_view(self.nav.navigation()[next].view.clone())?;
                }
            }
            Focus::Articles => self.move_article(forward, distance)?,
            Focus::Reader => self.move_reader(forward, distance),
            Focus::Annotations => self.move_annotation(forward, distance),
        }
        Ok(())
    }

    fn move_article(&mut self, forward: bool, distance: usize) -> Result<()> {
        let Some(position) = self.nav.next_article_position(forward, distance)
        else {
            return Ok(());
        };
        self.pin_article_viewport();
        let preview = matches!(self.interaction.mode(), super::Mode::Input(input)
            if matches!(input.kind, super::InputKind::Search));
        if !preview {
            self.save_reading_position()?;
        }
        if self.nav.move_article_cursor(position, preview) {
            if preview {
                self.filter_preview()?;
            } else {
                self.filter()?;
            }
        } else if !preview {
            self.load_current_deferred()?;
        }
        Ok(())
    }

    pub(crate) fn change_focus(&mut self, forward: bool) {
        self.prepare_panel_focus();
        let panes = self.expanded_panes();
        if panes.is_empty() {
            self.layout.show_pane(crate::model::Pane::Reader, true);
            self.layout.focus_pane(Focus::Reader);
            return;
        }
        let index = panes
            .iter()
            .position(|pane| *pane == self.layout.focus())
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % panes.len()
        } else {
            (index + panes.len() - 1) % panes.len()
        };
        self.layout.focus_pane(panes[next]);
    }

    pub(super) fn prepare_panel_focus(&mut self) {
        self.layout.set_zen(false);
        self.layout.set_fullscreen(false);
        self.layout.remember_fullscreen_focus(None);
        self.reader.cancel_selection();
        self.interaction.vim_input().clear_input();
    }

    pub(crate) fn open_reader(&mut self) -> Result<()> {
        if self.current().is_none() {
            return Ok(());
        }
        self.nav
            .retain_article(self.current().map(|article| article.id));
        self.layout.focus_pane(Focus::Reader);
        self.layout.show_pane(crate::model::Pane::Reader, true);
        Ok(())
    }

    pub(super) fn refresh_read_counts(&mut self) -> Result<()> {
        let navigation = super::catalog_worker::read_counts(
            &self.store,
            self.nav.library(),
            self.layout.workspace().last_visit,
            self.nav.navigation_sections().catalog(),
        )?;
        self.install_navigation(navigation);
        Ok(())
    }

    /// Jump from global search or a restored session to an article of any age.
    ///
    /// # Errors
    /// Returns missing article or SQLite errors.
    pub fn jump_article(&mut self, article: i64) -> Result<()> {
        self.suspend_notebook_for_source();
        self.save_reading_position()?;
        self.nav.begin_jump();
        if self.navigation_worker.is_some() {
            self.defer_navigation(super::navigation_worker::Request::Jump {
                query: self.article_query(),
                article,
            });
        } else {
            let position = self.store.article_position(article)?;
            self.nav.jump_position(position);
            self.filter_now()?;
        }
        self.layout.show_pane(crate::model::Pane::Reader, true);
        self.layout.focus_pane(Focus::Reader);
        Ok(())
    }

    pub(crate) fn restore_article(&mut self, article: i64) -> Result<()> {
        // Another session may delete or restore the saved target. Keep the
        // current destination instead of reopening an article outside it.
        if self.store.is_deleted(article)? != (*self.nav.view() == View::Trash)
        {
            return Ok(());
        }
        if let Some(position) = self
            .nav
            .library()
            .articles
            .iter()
            .position(|item| item.id == article)
        {
            self.nav.select_article(position);
            return self.load_current();
        }
        self.jump_article(article)
    }
}

pub(super) fn sensitive(
    view: &View,
    lists: &[crate::model::SmartList],
    changes: crate::search::Changes,
) -> Result<bool> {
    use crate::search::Changes;
    if changes == Changes::default() {
        return Ok(false);
    }
    match view {
        View::Inbox | View::Read => Ok(changes.intersects(Changes::READ)),
        View::Starred => Ok(changes.intersects(Changes::FAVORITE)),
        View::Queue => Ok(changes.intersects(Changes::QUEUE)),
        View::Annotated => Ok(changes.intersects(Changes::NOTES)),
        View::Collection(_) => Ok(changes.intersects(Changes::MEMBERSHIPS)),
        View::Recommended => Ok(true),
        View::Smart(id) => {
            let Some(list) = lists.iter().find(|list| list.id == *id) else {
                return Ok(false);
            };
            if list.filter.paused {
                return Ok(false);
            }
            Ok((list.filter.unread && changes.intersects(Changes::READ))
                || (list.filter.starred
                    && changes.intersects(Changes::FAVORITE))
                || crate::search::parse(&list.filter.text)?.depends_on(changes))
        }
        _ => Ok(false),
    }
}

impl App {
    pub(super) fn install_metadata_page(
        &mut self,
        query: ArticleQuery,
        page: ArticlePage,
    ) -> Result<()> {
        if self.nav.install_metadata_page(query, page) {
            self.load_current()?;
        }
        Ok(())
    }
}

pub(super) fn pending_navigation(
    library: &crate::model::Library,
    last_visit: i64,
) -> Vec<NavItem> {
    catalog_destinations(library, last_visit)
        .into_iter()
        .map(|(view, title, group)| NavItem {
            view,
            title,
            group,
            count: 0,
        })
        .collect()
}

pub(super) fn catalog_navigation(
    store: &crate::store::Store,
    library: &crate::model::Library,
    last_visit: i64,
) -> Result<Vec<NavItem>> {
    let mut navigation = pending_navigation(library, last_visit);
    let views = navigation
        .iter()
        .map(|item| item.view.clone())
        .collect::<Vec<_>>();
    for (item, count) in navigation.iter_mut().zip(store.view_counts(&views)?) {
        item.count = count;
    }
    Ok(navigation)
}

fn catalog_destinations(
    library: &crate::model::Library,
    last_visit: i64,
) -> Vec<(View, String, &'static str)> {
    let mut destinations = vec![
        (View::Inbox, "Inbox".into(), "LIBRARY"),
        (View::All, "All articles".into(), "LIBRARY"),
        (View::Queue, "Read next".into(), "LIBRARY"),
        (View::Starred, "Favorites".into(), "LIBRARY"),
        (View::Read, "Read".into(), "LIBRARY"),
        (View::Annotated, "Notebook".into(), "LIBRARY"),
        (View::Trash, "Trash".into(), "LIBRARY"),
        (View::Continue, "Continue reading".into(), "LIBRARY"),
        (
            View::SinceLastVisit(last_visit),
            "Since last visit".into(),
            "LIBRARY",
        ),
        (View::Grouped, "Grouped articles".into(), "LIBRARY"),
        (View::Recommended, "For you".into(), "LIBRARY"),
    ];
    destinations.extend(library.collections.iter().map(|list| {
        (View::Collection(list.id), list.name.clone(), "YOUR FOLDERS")
    }));
    destinations.extend(
        library.smart_lists.iter().map(|list| {
            (View::Smart(list.id), list.name.clone(), "SMART LISTS")
        }),
    );
    destinations.extend(
        library
            .feeds
            .iter()
            .filter(|feed| feed.server.is_none())
            .map(|feed| (View::Feed(feed.id), feed.title.clone(), "FEEDS")),
    );
    destinations.extend(super::servers::destinations(library));
    destinations
}

impl App {
    /// Select a visible cached sidebar destination without changing article scope.
    ///
    /// # Errors
    /// Returns an error when the destination is hidden or no longer available.
    pub fn select_library_destination(
        &mut self,
        destination: &View,
    ) -> Result<()> {
        self.nav.select_library_destination(destination)
    }

    /// Resize metadata pages, preserving the captured identity or pending cursor.
    /// In-flight previews and source jumps retain their read intent and arrival.
    ///
    /// # Errors
    /// Returns checkpoint or metadata read failures.
    pub fn set_page_size(&mut self, size: usize) -> Result<()> {
        if self.nav.page_size() == size.clamp(1, 500) {
            return Ok(());
        }
        let article = if self.nav.navigation_loading() {
            self.navigation_worker.as_ref().and_then(
                super::navigation_worker::NavigationWorker::captured_article,
            )
        } else {
            self.current().map(|article| article.id)
        };
        self.save_reading_position()?;
        self.nav.resize_page(size);
        if self.navigation_worker.is_some() {
            self.resize_navigation(article);
        } else {
            self.filter_now()?;
            if let Some(article) = article {
                self.restore_page_selection(article)?;
            }
        }
        Ok(())
    }
}
