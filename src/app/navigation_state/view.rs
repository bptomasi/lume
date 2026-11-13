//! Navigation owner constructs a borrowed projection.

use super::NavigationState;
pub use crate::presentation::navigation::NavigationView;

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
