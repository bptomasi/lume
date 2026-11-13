//! Borrowed navigation metadata and the shared filtered sidebar row projection.

use crate::model::{Library, View};
use std::collections::BTreeSet;

/// Borrowed navigation caches; creating this view performs no work or I/O.
#[derive(Clone, Copy)]
pub struct NavigationView<'a> {
    /// In-memory sidebar name filter, independent of article search.
    pub library_query: &'a str,
    pub(crate) matches: &'a [usize],
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

/// A selectable navigation row.
#[derive(Clone)]
pub struct NavItem {
    /// Destination filter.
    pub view: View,
    /// Display name.
    pub title: String,
    /// Section label; repeated labels are grouped by the renderer.
    pub group: &'static str,
    /// Number of articles across the complete destination.
    pub count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Row {
    Blank,
    Heading(&'static str),
    Item(usize),
}

pub(crate) fn window(
    navigation: NavigationView<'_>,
    height: u16,
) -> impl Iterator<Item = Row> + '_ {
    let (first, skip) = window_start(navigation, height);
    rows(navigation, first).skip(skip).take(usize::from(height))
}

/// Stable sidebar origin, including skipped heading rows.
pub fn window_start(
    navigation: NavigationView<'_>,
    height: u16,
) -> (usize, usize) {
    let selected = navigation.position(navigation.nav_selected).unwrap_or(0);
    if selected < navigation.navigation_first {
        return lookbehind(navigation, selected, 0);
    }
    let first = navigation.navigation_first.min(selected);
    let skip = if first == navigation.navigation_first {
        navigation
            .navigation_skip
            .min(heading_rows(navigation, first))
    } else {
        0
    };
    if rows(navigation, first)
        .skip(skip)
        .take(usize::from(height))
        .any(|row| row == Row::Item(navigation.nav_selected))
    {
        return (first, skip);
    }
    lookbehind(navigation, selected, usize::from(height).saturating_sub(1))
}

fn lookbehind(
    navigation: NavigationView<'_>,
    mut first: usize,
    mut remaining: usize,
) -> (usize, usize) {
    let mut skip = heading_rows(navigation, first);
    loop {
        if remaining <= skip {
            skip -= remaining;
            break;
        }
        remaining -= skip;
        if first == 0 {
            skip = 0;
            break;
        }
        first -= 1;
        skip = heading_rows(navigation, first) + 1;
    }
    (first, skip)
}

fn heading_rows(navigation: NavigationView<'_>, position: usize) -> usize {
    let Some(index) = navigation.indices().nth(position) else {
        return 0;
    };
    let previous = position
        .checked_sub(1)
        .and_then(|position| navigation.indices().nth(position))
        .map_or("", |index| navigation.navigation[index].group);
    if navigation.navigation[index].group == previous {
        return 0;
    }
    if matches!(
        navigation.navigation[index].view,
        crate::model::View::Group(_)
    ) {
        1
    } else {
        2
    }
}

fn rows(
    navigation: NavigationView<'_>,
    first: usize,
) -> impl Iterator<Item = Row> + '_ {
    let mut group = first
        .checked_sub(1)
        .and_then(|position| navigation.indices().nth(position))
        .map_or("", |index| navigation.navigation[index].group);
    navigation.indices().skip(first).flat_map(move |index| {
        let item = &navigation.navigation[index];
        let changed_group = item.group != group;
        group = item.group;
        [
            changed_group.then_some(Row::Blank),
            (changed_group
                && !matches!(item.view, crate::model::View::Group(_)))
            .then_some(Row::Heading(group)),
            Some(Row::Item(index)),
        ]
        .into_iter()
        .flatten()
    })
}
