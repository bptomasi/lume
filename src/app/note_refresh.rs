//! Local note changes refresh annotations without reloading publisher content.
//!
//! The source renderer caches its unmarked base layout. Reapplying private
//! marks must not parse Markdown, reread article documents or recount every
//! smart list before the next input frame.

use anyhow::Result;

use super::{App, View};

impl App {
    pub(super) fn refresh_notes(&mut self, article: i64) -> Result<()> {
        let comments = self.store.comments(article)?;
        self.install_note_changes(article, comments);
        Ok(())
    }

    pub(super) fn install_note_changes(
        &mut self,
        article: i64,
        comments: Vec<crate::model::Comment>,
    ) {
        self.invalidate_notebook();
        self.refresh_note_catalog();
        let count = comments.len();
        self.update_note_counts(article, count);
        self.invalidate_reading_refresh();
        if self.current().is_none_or(|item| item.id != article) {
            return;
        }
        let snapshot = self.capture_reader();
        self.comments = comments;
        self.comment_selected =
            self.comment_selected.min(count.saturating_sub(1));
        self.rebuild_annotations();
        if self.projecting_navigation {
            return;
        }
        self.wrap_reader();
        self.restore_reader(snapshot);
        self.clamp_cursor();
        self.ensure_cursor_visible();
    }
    fn update_note_counts(&mut self, article: i64, count: usize) {
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
