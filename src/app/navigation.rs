//! SQLite-backed navigation with bounded pages and no historical cutoff.
//!
//! Destination changes reuse catalog counts. Personal edits and collection
//! completion refresh those counts; moving a cursor does not mutate them.

use anyhow::Result;

use crate::model::{ArticlePage, ArticleQuery};

use super::{App, Focus, NavItem, View};

impl App {
    pub(crate) fn rebuild_navigation(&mut self) -> Result<()> {
        let items = catalog_navigation(
            &self.store,
            &self.library,
            self.workspace.last_visit,
        )?;
        self.install_navigation(items);
        Ok(())
    }

    pub(super) fn install_navigation(&mut self, items: Vec<NavItem>) {
        self.navigation = items;
        self.customize_navigation();
        self.nav_selected = self
            .navigation
            .iter()
            .position(|item| item.view.same_destination(&self.view))
            .unwrap_or(1);
        self.view = self.navigation[self.nav_selected].view.clone();
    }

    pub(crate) fn filter(&mut self) -> Result<()> {
        let query = self.article_query();
        if self.navigation_worker.is_some() {
            self.defer_navigation(super::navigation_worker::Request::Page {
                query,
                selected: self.selected,
            });
            self.library.articles.clear();
            self.visible.clear();
            return Ok(());
        }
        self.filter_now()
    }

    pub(super) fn filter_now(&mut self) -> Result<()> {
        let query = self.article_query();
        let page = self.store.query_articles(&query)?;
        self.page_total = page.total;
        if page.articles.is_empty() && page.total > 0 {
            self.page_offset =
                (page.total - 1) / self.page_limit() * self.page_limit();
            return self.filter_now();
        }
        self.apply_page(page)
    }

    pub(super) fn article_query(&self) -> ArticleQuery {
        ArticleQuery {
            view: self.view.clone(),
            text: self.search.clone(),
            offset: self.page_offset,
            limit: self.page_limit(),
            retained: self.retained_article,
        }
    }

    fn page_limit(&self) -> usize {
        // Match Store's bounded page contract before doing cursor arithmetic.
        self.page_size.clamp(1, 500)
    }

    fn apply_page(&mut self, page: crate::model::ArticlePage) -> Result<()> {
        self.page_total = page.total;
        self.library.articles = page.articles;
        self.visible = (0..self.library.articles.len()).collect();
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
        self.load_current()
    }

    pub(super) fn restore_page_selection(
        &mut self,
        article: i64,
    ) -> Result<()> {
        if !self.library.articles.iter().any(|item| item.id == article) {
            let Some((offset, page)) =
                self.store.page_containing(&self.article_query(), article)?
            else {
                return Ok(());
            };
            self.page_offset = offset;
            self.apply_page(page)?;
        }
        if let Some(position) = self
            .library
            .articles
            .iter()
            .position(|item| item.id == article)
        {
            self.selected = position;
            self.load_current()?;
        }
        Ok(())
    }

    /// Switch destinations and release the retained inbox selection.
    ///
    /// # Errors
    /// Returns SQLite or content-loading errors.
    pub fn set_view(&mut self, view: View) -> Result<()> {
        self.close_notebook();
        self.save_reading_position()?;
        self.retained_article = None;
        self.nav_selected = self
            .navigation
            .iter()
            .position(|item| item.view.same_destination(&view))
            .unwrap_or(1);
        self.view = self.navigation[self.nav_selected].view.clone();
        self.page_offset = 0;
        self.selected = 0;
        self.search.clear();
        self.filter()
    }

    pub(crate) fn move_selection(
        &mut self,
        forward: bool,
        distance: usize,
    ) -> Result<()> {
        match self.focus {
            Focus::Library => {
                let next = shifted(
                    self.nav_selected,
                    self.navigation.len(),
                    forward,
                    distance,
                );
                if next != self.nav_selected {
                    self.set_view(self.navigation[next].view.clone())?;
                }
            }
            Focus::Articles => self.move_article(forward, distance)?,
            Focus::Reader => self.move_reader(forward, distance),
            Focus::Annotations => self.move_annotation(forward, distance),
        }
        Ok(())
    }

    fn move_article(&mut self, forward: bool, distance: usize) -> Result<()> {
        let previous = self.page_offset + self.selected;
        let position = shifted(previous, self.page_total, forward, distance);
        if position == previous {
            return Ok(());
        }
        self.save_reading_position()?;
        let page = position / self.page_limit() * self.page_limit();
        self.selected = position % self.page_limit();
        if page != self.page_offset || self.library.articles.is_empty() {
            self.page_offset = page;
            self.filter()?;
        } else {
            self.load_current_deferred()?;
        }
        Ok(())
    }

    pub(crate) fn change_focus(&mut self, forward: bool) {
        self.workspace.zen = false;
        self.workspace.fullscreen = false;
        self.fullscreen_focus = None;
        self.selection = None;
        self.vim.clear_input();
        let panes = self.expanded_panes();
        if panes.is_empty() {
            self.workspace.show_reader = true;
            self.focus = Focus::Reader;
            return;
        }
        let index = panes
            .iter()
            .position(|pane| *pane == self.focus)
            .unwrap_or(0);
        let next = if forward {
            (index + 1) % panes.len()
        } else {
            (index + panes.len() - 1) % panes.len()
        };
        self.focus = panes[next];
    }

    pub(crate) fn open_reader(&mut self) -> Result<()> {
        let Some(index) = self.visible.get(self.selected).copied() else {
            return Ok(());
        };
        let article = &mut self.library.articles[index];
        let changed = article.unread;
        if changed {
            let id = article.id;
            self.submit_read_state(std::iter::once(id).collect(), false)?;
        }
        self.retained_article = self.current().map(|article| article.id);
        self.focus = Focus::Reader;
        self.workspace.show_reader = true;
        if changed && self.personal_writes.is_none() {
            self.refresh_read_counts()?;
        }
        Ok(())
    }

    pub(super) fn refresh_read_counts(&mut self) -> Result<()> {
        for item in &mut self.navigation {
            if read_sensitive(&item.view, &self.library.smart_lists)? {
                item.count = self.store.view_count(&item.view)?;
            }
        }
        Ok(())
    }

    /// Jump from global search or a restored session to an article of any age.
    ///
    /// # Errors
    /// Returns missing article or SQLite errors.
    pub fn jump_article(&mut self, article: i64) -> Result<()> {
        self.suspend_notebook_for_source();
        self.save_reading_position()?;
        self.view = View::All;
        self.search.clear();
        if self.navigation_worker.is_some() {
            self.defer_navigation(super::navigation_worker::Request::Jump {
                query: self.article_query(),
                article,
            });
        } else {
            let position = self.store.article_position(article)?;
            self.page_offset = position / self.page_limit() * self.page_limit();
            self.selected = position % self.page_limit();
            self.filter_now()?;
        }
        self.nav_selected = self
            .navigation
            .iter()
            .position(|item| item.view == View::All)
            .unwrap_or(0);
        self.workspace.show_reader = true;
        self.focus = Focus::Reader;
        Ok(())
    }

    pub(crate) fn restore_article(&mut self, article: i64) -> Result<()> {
        // Another session may delete or restore the saved target. Keep the
        // current destination instead of reopening an article outside it.
        if self.store.is_deleted(article)? != (self.view == View::Trash) {
            return Ok(());
        }
        if let Some(position) = self
            .library
            .articles
            .iter()
            .position(|item| item.id == article)
        {
            self.selected = position;
            return self.load_current();
        }
        self.jump_article(article)
    }
}

fn shifted(
    position: usize,
    len: usize,
    forward: bool,
    distance: usize,
) -> usize {
    if forward {
        return position.saturating_add(distance).min(len.saturating_sub(1));
    }
    position.saturating_sub(distance)
}

fn read_sensitive(
    view: &View,
    lists: &[crate::model::SmartList],
) -> Result<bool> {
    match view {
        View::Inbox | View::Read | View::Recommended => Ok(true),
        View::Smart(id) => {
            let Some(list) = lists.iter().find(|list| list.id == *id) else {
                return Ok(false);
            };
            Ok(list.filter.unread
                || crate::search::parse(&list.filter.text)?
                    .depends_on_read_state())
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
        if query.view != self.view
            || query.text != self.search
            || query.offset != self.page_offset
            || self.navigation_loading
        {
            return Ok(());
        }
        let selected = self.current().map(|article| article.id);
        self.selected = page
            .articles
            .iter()
            .position(|article| Some(article.id) == selected)
            .unwrap_or(
                self.selected.min(page.articles.len().saturating_sub(1)),
            );
        self.page_total = page.total;
        self.library.articles = page.articles;
        self.visible = (0..self.library.articles.len()).collect();
        if self.current().map(|article| article.id) != selected {
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
    catalog_destinations(library, last_visit)
        .into_iter()
        .map(|(view, title, group)| {
            Ok(NavItem {
                count: store.view_count(&view)?,
                view,
                title,
                group,
            })
        })
        .collect()
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
            .map(|feed| (View::Feed(feed.id), feed.title.clone(), "FEEDS")),
    );
    destinations
}
