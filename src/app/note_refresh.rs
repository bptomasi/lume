//! Local note changes refresh annotations without reloading publisher content.
//!
//! The source renderer caches its unmarked base layout. Reapplying private
//! marks must not parse Markdown, reread article documents or recount every
//! smart list before the next input frame.

use anyhow::Result;

use super::App;

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
        self.invalidate_reading_refresh();
        self.refresh_note_catalog();
        let count = comments.len();
        self.nav.update_note_counts(article, count);
        if self.current().is_none_or(|item| item.id != article)
            || self.reading_article() != Some(article)
        {
            return;
        }
        let snapshot = self.capture_reader();
        self.reader.replace_comments(comments);
        if self.nav.projecting_navigation() {
            return;
        }
        self.wrap_reader();
        self.restore_reader(snapshot);
        self.clamp_cursor();
        self.ensure_cursor_visible();
    }
}

#[cfg(test)]
mod tests;
