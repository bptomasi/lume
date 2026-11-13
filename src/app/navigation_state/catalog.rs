//! Catalog responsibilities owned by navigation.

use super::{BTreeSet, Library, NavItem, NavigationState, View};

impl NavigationState {
    pub(in crate::app) fn project_navigation(
        &mut self,
        workspace: &crate::model::Workspace,
    ) {
        self.navigation = self
            .navigation_sections
            .project(&workspace.collapsed_groups);
        self.nav_selected = self
            .navigation
            .iter()
            .position(|item| item.view.same_destination(&self.view))
            .unwrap_or(1);
        self.view = self.navigation[self.nav_selected].view.clone();
        self.refresh_library_search();
    }

    pub(in crate::app) fn install_navigation(
        &mut self,
        items: Vec<NavItem>,
        workspace: &crate::model::Workspace,
        editing: bool,
    ) {
        let selected = editing
            .then(|| self.navigation.get(self.nav_selected))
            .flatten()
            .map(|item| item.view.clone());
        self.navigation_sections
            .install(items, workspace, &self.library);
        self.project_navigation(workspace);
        if let Some(selected) = selected
            && let Some(index) = self
                .navigation
                .iter()
                .position(|item| item.view.same_destination(&selected))
            && self.navigation_position(index).is_some()
        {
            self.nav_selected = index;
        }
    }

    pub(in crate::app) fn replace_catalog(&mut self, mut library: Library) {
        library.articles = std::mem::take(&mut self.library.articles);
        self.library = library;
    }

    pub(in crate::app) fn customize(
        &mut self,
        workspace: &crate::model::Workspace,
    ) {
        self.navigation_sections.customize(workspace, &self.library);
    }

    pub(in crate::app) fn overlay_edit(
        &mut self,
        articles: &BTreeSet<i64>,
        edit: &crate::model::BulkEdit,
    ) {
        for article in &mut self.library.articles {
            if articles.contains(&article.id) {
                crate::app::article_edits::overlay(article, edit);
            }
        }
    }

    pub(in crate::app) fn update_recommendation_count(
        &mut self,
        total: usize,
    ) -> bool {
        let Some(row) = self
            .navigation
            .iter_mut()
            .find(|row| row.view == View::Recommended)
        else {
            return false;
        };
        let changed = row.count != total;
        row.count = total;
        changed
    }

    pub(in crate::app) fn update_note_counts(
        &mut self,
        article: i64,
        count: usize,
    ) {
        if let Some(metadata) = self
            .library
            .articles
            .iter_mut()
            .find(|item| item.id == article)
        {
            let had_notes = metadata.comments > 0;
            metadata.comments = count;
            if had_notes != (count > 0)
                && let Some(row) = self
                    .navigation
                    .iter_mut()
                    .find(|row| row.view == View::Annotated)
            {
                row.count = if count > 0 {
                    row.count.saturating_add(1)
                } else {
                    row.count.saturating_sub(1)
                };
            }
        }
    }
}
