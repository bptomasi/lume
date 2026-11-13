//! Selection responsibilities owned by navigation.

use super::{BTreeSet, NavigationState, View};

impl NavigationState {
    pub(in crate::app) fn remove_deleted_metadata(&mut self, article: i64) {
        let selected = self.current().map(|item| item.id);
        let previous = self.library.articles.len();
        self.library.articles.retain(|item| item.id != article);
        if self.library.articles.len() == previous {
            return;
        }
        self.visible = (0..self.library.articles.len()).collect();
        self.selected = selected
            .filter(|id| *id != article)
            .and_then(|id| {
                self.library.articles.iter().position(|item| item.id == id)
            })
            .unwrap_or_else(|| {
                self.selected.min(self.visible.len().saturating_sub(1))
            });
        // Grouped results may replace the deleted representative with another
        // occurrence. Let the authoritative page determine its group count.
        if self.view != View::Grouped {
            self.page_total = self.page_total.saturating_sub(1);
        }
    }

    pub(in crate::app) fn clear_deleted_destination(&mut self) {
        self.clear_section();
        self.selected = 0;
        self.retained_article = None;
    }

    pub(in crate::app) fn clear_marks(&mut self) {
        self.marked.clear();
    }

    pub(in crate::app) fn mark_current(&mut self) -> anyhow::Result<()> {
        use anyhow::Context;
        let article = self.current().context("Select an article")?.id;
        if !self.marked.remove(&article) {
            anyhow::ensure!(
                self.marked.len() < 1000,
                "Selection limit is 1,000 articles"
            );
            self.marked.insert(article);
        }
        Ok(())
    }

    pub(in crate::app) fn mark_page(&mut self) -> anyhow::Result<()> {
        let selection: BTreeSet<_> = self
            .marked
            .iter()
            .copied()
            .chain(self.library.articles.iter().map(|article| article.id))
            .collect();
        anyhow::ensure!(
            selection.len() <= 1000,
            "Selection limit is 1,000 articles"
        );
        self.marked = selection;
        Ok(())
    }
}
