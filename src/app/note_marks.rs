//! Width-specific note hits use cached readable bytes, never publisher guesses.
//!
//! Only safely resolved current-source quotations get passage markers. General
//! and unplaced notes remain actionable in their appended private-comment rows.
//! Reflow rebuilds these caches and cancels transient hover/delete targets.

use super::App;

impl App {
    /// Note under the reader cursor or selected in the annotation panel.
    pub fn cursor_note(&self) -> Option<i64> {
        match self.layout.focus() {
            super::Focus::Annotations => self
                .reader
                .view()
                .comments
                .get(self.reader.view().comment_selected)
                .map(|note| note.id),
            super::Focus::Reader if !self.article_preparing() => {
                self.note_at_offset(self.cursor_offset())
            }
            _ => None,
        }
    }
}
