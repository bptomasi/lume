//! Borrowed notebook caches contain no workers or storage authority.

use super::{
    NoteCollection, NoteContext, NotePage, NoteSummary, Notebook, NotebookState,
};

/// Cached notebook presentation, independent of article navigation.
#[derive(Clone, Copy)]
pub struct NotebookView<'a> {
    /// Note query, independent of article metadata search.
    pub query: &'a str,
    /// Cached bounded note page, retained during replacement reads.
    pub page: &'a NotePage,
    /// Absolute requested note-page offset.
    pub offset: usize,
    /// Selected position within the cached note page.
    pub selected: usize,
    /// Cached user-authored queries after four built-in destinations.
    pub collections: &'a [NoteCollection],
    /// Selected collection position, including built-in destinations.
    pub collection: usize,
    /// Selected position in incoming then outgoing note links.
    pub link_selected: usize,
    /// First visible prepared context row.
    pub scroll: usize,
    /// Most recent acknowledged explicit note queries.
    pub recent: &'a [String],
    /// Captured note identities selected for synthesis.
    pub marked: &'a std::collections::BTreeSet<i64>,
    /// A replacement note-list read is pending.
    pub loading: bool,
    /// The selected note source context is pending.
    pub context_loading: bool,
    /// Validation or read failure kept outside user-authored text.
    pub error: &'a str,
    current: Option<&'a NoteSummary>,
    context: Option<&'a NoteContext>,
}

impl<'a> NotebookView<'a> {
    // Array sizes in the renderer enforce these shared scroll-row counts.
    pub(crate) const NOTE_HEADING_ROWS: usize = 4;
    pub(crate) const SOURCE_HEADING_ROWS: usize = 3;

    /// Selected preview only when it belongs to the current query and page.
    pub fn current(&self) -> Option<&'a NoteSummary> {
        self.current
    }

    /// Context belongs to the exact selected note identity.
    pub fn current_context(&self) -> Option<&'a NoteContext> {
        self.context
    }

    /// Visible note cards, shared by rendering and pointer hits.
    pub fn window(&self, height: u16) -> (usize, usize) {
        let capacity = usize::from(height / 5).max(1);
        let first = self
            .selected
            .saturating_sub(capacity / 2)
            .min(self.page.notes.len().saturating_sub(capacity));
        (first, capacity)
    }

    /// Prepared context rows, including four note and three source headings.
    pub fn context_scroll_limit(&self, height: u16) -> usize {
        let Some(context) = self.current_context() else {
            return 0;
        };
        let rows = Self::NOTE_HEADING_ROWS
            + context.lines.len()
            + Self::SOURCE_HEADING_ROWS
            + context.source_lines.len();
        rows.saturating_sub(usize::from(height.saturating_sub(2).max(1)))
    }

    /// First visible collection row.
    pub fn collection_start(&self, height: u16) -> usize {
        self.collection
            .saturating_sub(usize::from(height).saturating_sub(1))
    }
}

impl NotebookState {
    pub(in crate::app) fn view(&self) -> Option<NotebookView<'_>> {
        self.active.as_ref().map(|book| book.view())
    }
}

impl Notebook {
    pub(super) fn view(&self) -> NotebookView<'_> {
        NotebookView {
            query: &self.query,
            page: &self.page,
            offset: self.offset,
            selected: self.selected,
            collections: &self.collections,
            collection: self.collection,
            link_selected: self.link_selected,
            scroll: self.scroll,
            recent: &self.recent,
            marked: &self.marked,
            loading: self.loading,
            context_loading: self.context_loading,
            error: &self.error,
            current: self.current(),
            context: self.current_context(),
        }
    }
}
