//! View responsibilities owned by navigation.

use super::{BTreeSet, Library, NavItem, NavigationState, View};

/// Borrowed navigation caches; creating this view performs no work or I/O.
#[derive(Clone, Copy)]
pub struct NavigationView<'a> {
    /// In-memory sidebar name filter, independent of article search.
    pub library_query: &'a str,
    matches: &'a [usize],
    /// Body-free catalog metadata and the bounded current article page.
    pub library: &'a Library,
    /// Cached destination and section rows in display order.
    pub navigation: &'a [NavItem],
    /// Selected catalog row identity, before sidebar filtering.
    pub nav_selected: usize,
    /// First filtered sidebar position pinned by the last input frame.
    pub navigation_first: usize,
    /// Leading heading rows skipped within the first sidebar position.
    pub navigation_skip: usize,
    /// Active article destination; sidebar highlighting may differ.
    pub view: &'a View,
    /// Metadata search text for the current article destination.
    pub search: &'a str,
    /// Matching indices into the cached library article page.
    pub visible: &'a [usize],
    /// Selected position in visible article indices.
    pub selected: usize,
    /// First visible article position pinned by input.
    pub articles_first: usize,
    /// Absolute offset of the current or requested article page.
    pub page_offset: usize,
    /// Total matching article rows, including rows outside the page.
    pub page_total: usize,
    /// Requested page capacity, bounded to 1–500 article rows.
    pub page_size: usize,
    /// A replacement selection is pending; cached content may be stale.
    pub navigation_loading: bool,
    /// Captured article identities selected for a reversible bulk action.
    pub marked: &'a BTreeSet<i64>,
}

impl NavigationState {
    pub(in crate::app) fn presentation(&self) -> NavigationView<'_> {
        NavigationView {
            library_query: &self.library_search.query,
            matches: &self.library_search.matches,
            library: &self.library,
            navigation: &self.navigation,
            nav_selected: self.nav_selected,
            navigation_first: self.navigation_first,
            navigation_skip: self.navigation_skip,
            view: &self.view,
            search: &self.search,
            visible: &self.visible,
            selected: self.selected,
            articles_first: self.articles_first,
            page_offset: self.page_offset,
            page_total: self.page_total,
            page_size: self.page_size,
            navigation_loading: self.navigation_loading,
            marked: &self.marked,
        }
    }
}

impl<'a> NavigationView<'a> {
    /// Catalog row identities in the cached sidebar filter's display order.
    pub fn indices(
        self,
    ) -> impl DoubleEndedIterator<Item = usize> + Clone + 'a {
        let count = if self.library_query.is_empty() {
            self.navigation.len()
        } else {
            0
        };
        (0..count).chain(self.matches.iter().copied())
    }

    /// Position of a catalog row identity in the current sidebar filter.
    pub fn position(&self, index: usize) -> Option<usize> {
        if self.library_query.is_empty() {
            self.navigation.get(index).map(|_| index)
        } else {
            self.matches.binary_search(&index).ok()
        }
    }
}
