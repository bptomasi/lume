//! Page responsibilities owned by navigation.

use super::{NavigationState, View};

impl NavigationState {
    pub(in crate::app) fn article_query(&self) -> crate::model::ArticleQuery {
        crate::model::ArticleQuery {
            view: self.view.clone(),
            text: self.search.clone(),
            offset: self.page_offset,
            limit: self.page_limit(),
            retained: self.retained_article,
        }
    }

    pub(in crate::app) fn page_limit(&self) -> usize {
        // Match Store's bounded page contract before doing cursor arithmetic.
        self.page_size.clamp(1, 500)
    }

    pub(in crate::app) fn current(&self) -> Option<&crate::model::Article> {
        self.visible
            .get(self.selected)
            .and_then(|index| self.library.articles.get(*index))
    }

    pub(in crate::app) fn install_page(
        &mut self,
        page: crate::model::ArticlePage,
    ) {
        self.page_total = page.total;
        self.library.articles = page.articles;
        self.visible = (0..self.library.articles.len()).collect();
        self.selected = self.selected.min(self.visible.len().saturating_sub(1));
    }

    pub(in crate::app) fn install_window(
        &mut self,
        offset: usize,
        page: crate::model::ArticlePage,
        selected: usize,
    ) {
        self.page_offset = offset;
        self.selected = selected;
        self.install_page(page);
    }

    pub(in crate::app) fn clear_page(&mut self) {
        self.library.articles.clear();
        self.visible.clear();
    }

    pub(in crate::app) fn clear_section(&mut self) {
        self.page_total = 0;
        self.clear_page();
    }

    pub(in crate::app) fn select_destination(&mut self, view: &View) {
        self.retained_article = None;
        self.nav_selected = self
            .navigation
            .iter()
            .position(|item| item.view.same_destination(view))
            .unwrap_or(1);
        if let Some(item) = self.navigation.get(self.nav_selected) {
            self.view = item.view.clone();
        }
        self.reset_page();
        self.search.clear();
    }

    pub(in crate::app) fn reset_page(&mut self) {
        self.page_offset = 0;
        self.selected = 0;
        self.articles_first = 0;
    }

    pub(in crate::app) fn preview_query(&mut self, search: String) {
        self.search = search;
        self.reset_page();
    }

    pub(in crate::app) fn restore_query(&mut self, search: String) {
        self.search = search;
        self.page_offset = 0;
    }

    pub(in crate::app) fn clear_query(&mut self) {
        self.retained_article = None;
        self.restore_query(String::new());
    }

    pub(in crate::app) fn select_article(&mut self, position: usize) {
        self.selected = position.min(self.visible.len().saturating_sub(1));
    }

    pub(in crate::app) fn retain_article(&mut self, article: Option<i64>) {
        self.retained_article = article;
    }

    pub(in crate::app) fn pin_navigation(&mut self, first: usize, skip: usize) {
        self.navigation_first = first;
        self.navigation_skip = skip;
    }

    pub(in crate::app) fn pin_articles(&mut self, first: usize) {
        self.articles_first = first;
    }

    pub(in crate::app) fn install_offset_page(
        &mut self,
        offset: usize,
        page: crate::model::ArticlePage,
    ) {
        self.install_window(offset, page, self.selected);
    }

    /// Choose the target for an upcoming catalog projection or section fold.
    /// The caller reconciles the existing article page after projection.
    pub(in crate::app) fn project_destination(&mut self, view: View) {
        self.view = view;
    }

    pub(in crate::app) fn next_article_position(
        &self,
        forward: bool,
        distance: usize,
    ) -> Option<usize> {
        let current = self.page_offset + self.selected;
        let next = shifted(current, self.page_total, forward, distance);
        (next != current).then_some(next)
    }

    // Keep a requested cursor even when its page has not arrived yet. Clamping
    // against retained metadata here would lose rapid cross-page movement.
    pub(in crate::app) fn move_article_cursor(
        &mut self,
        position: usize,
        preview: bool,
    ) -> bool {
        let page = position / self.page_limit() * self.page_limit();
        self.selected = position % self.page_limit();
        let request = page != self.page_offset
            || self.library.articles.is_empty()
            || (preview && self.navigation_loading);
        if request {
            self.articles_first = 0;
            self.page_offset = page;
        }
        request
    }

    pub(in crate::app) fn begin_jump(&mut self) {
        self.view = View::All;
        self.search.clear();
        self.nav_selected = self
            .navigation
            .iter()
            .position(|item| item.view == View::All)
            .unwrap_or(0);
    }

    pub(in crate::app) fn jump_position(&mut self, position: usize) {
        self.page_offset = position / self.page_limit() * self.page_limit();
        self.selected = position % self.page_limit();
    }

    pub(in crate::app) fn install_metadata_page(
        &mut self,
        query: crate::model::ArticleQuery,
        page: crate::model::ArticlePage,
    ) -> bool {
        if query.view != self.view
            || query.text != self.search
            || query.offset != self.page_offset
            || query.limit != self.page_limit()
            || query.retained != self.retained_article
            || self.navigation_loading
        {
            return false;
        }
        let selected = self.current().map(|article| article.id);
        self.selected = page
            .articles
            .iter()
            .position(|article| Some(article.id) == selected)
            .unwrap_or(
                self.selected.min(page.articles.len().saturating_sub(1)),
            );
        self.install_page(page);
        self.current().map(|article| article.id) != selected
    }

    pub(in crate::app) fn select_library_destination(
        &mut self,
        destination: &View,
    ) -> anyhow::Result<()> {
        use anyhow::Context;
        let index = self
            .navigation
            .iter()
            .position(|item| item.view.same_destination(destination))
            .filter(|index| self.navigation_position(*index).is_some())
            .context("No matching visible library destination")?;
        self.nav_selected = index;
        Ok(())
    }

    pub(in crate::app) fn resize_page(&mut self, size: usize) {
        let position = self.page_offset + self.selected;
        self.page_size = size.clamp(1, 500);
        self.jump_position(position);
        self.articles_first = 0;
    }
}

fn shifted(
    position: usize,
    len: usize,
    forward: bool,
    distance: usize,
) -> usize {
    if forward {
        position.saturating_add(distance).min(len.saturating_sub(1))
    } else {
        position.saturating_sub(distance)
    }
}
