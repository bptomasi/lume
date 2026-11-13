//! Cached library projection and identity-stable navigation transitions.
//!
//! The owner keeps page metadata, selected identities and loading state together.
//! Presentation borrows existing caches and cannot access workers or storage.

mod catalog;
mod loading;
mod page;
mod search;
mod selection;
mod view;

pub use view::NavigationView;

use std::collections::BTreeSet;

use super::{NavItem, navigation_sections::Sections};
use crate::model::{Library, View};

pub(super) struct NavigationState {
    library: Library,
    navigation: Vec<NavItem>,
    navigation_sections: Sections,
    library_search: LibrarySearch,
    nav_selected: usize,
    navigation_first: usize,
    navigation_skip: usize,
    view: View,
    search: String,
    visible: Vec<usize>,
    selected: usize,
    articles_first: usize,
    retained_article: Option<i64>,
    page_offset: usize,
    page_total: usize,
    page_size: usize,
    catalog_loading: bool,
    library_revision: u64,
    projecting_navigation: bool,
    navigation_loading: bool,
    marked: BTreeSet<i64>,
}

#[derive(Default)]
struct LibrarySearch {
    query: String,
    names: Vec<String>,
    matches: Vec<usize>,
    before: Option<(String, super::View)>,
}

impl NavigationState {
    pub(super) fn new(
        library: Library,
        view: View,
        page_offset: usize,
        retained_article: Option<i64>,
        navigation_loading: bool,
    ) -> Self {
        Self {
            library,
            navigation: Vec::new(),
            navigation_sections: Sections::default(),
            library_search: LibrarySearch::default(),
            nav_selected: 0,
            navigation_first: 0,
            navigation_skip: 0,
            view,
            search: String::new(),
            visible: Vec::new(),
            selected: 0,
            articles_first: 0,
            retained_article,
            page_offset,
            page_total: 0,
            page_size: 200,
            catalog_loading: false,
            library_revision: 0,
            projecting_navigation: false,
            navigation_loading,
            marked: BTreeSet::new(),
        }
    }

    pub(super) fn library(&self) -> &Library {
        &self.library
    }

    pub(super) fn navigation(&self) -> &Vec<NavItem> {
        &self.navigation
    }

    pub(super) fn navigation_sections(&self) -> &Sections {
        &self.navigation_sections
    }

    pub(super) fn nav_selected(&self) -> usize {
        self.nav_selected
    }

    pub(super) fn view(&self) -> &View {
        &self.view
    }

    pub(super) fn search(&self) -> &String {
        &self.search
    }

    pub(super) fn selected(&self) -> usize {
        self.selected
    }

    pub(super) fn retained_article(&self) -> Option<i64> {
        self.retained_article
    }

    pub(super) fn page_offset(&self) -> usize {
        self.page_offset
    }

    pub(super) fn page_total(&self) -> usize {
        self.page_total
    }

    pub(super) fn page_size(&self) -> usize {
        self.page_size
    }

    pub(super) fn catalog_loading(&self) -> bool {
        self.catalog_loading
    }

    pub(super) fn library_revision(&self) -> u64 {
        self.library_revision
    }

    pub(super) fn projecting_navigation(&self) -> bool {
        self.projecting_navigation
    }

    pub(super) fn navigation_loading(&self) -> bool {
        self.navigation_loading
    }

    pub(super) fn marked(&self) -> &BTreeSet<i64> {
        &self.marked
    }
}
