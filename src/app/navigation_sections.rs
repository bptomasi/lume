//! Cached catalog projection for presentation-only sidebar edits.
//!
//! Counts are refreshed by the catalog owner after data changes. Folding only
//! projects these sections; it must never query SQLite or enqueue a page read.
//! Keep the unmodified catalog so unpinning restores each original section.

use std::collections::{HashMap, HashSet};

use crate::model::{Library, View, Workspace};

use super::NavItem;

#[derive(Default)]
pub(super) struct Sections {
    catalog: Vec<NavItem>,
    groups: Vec<Section>,
}

struct Section {
    name: &'static str,
    count: usize,
    children: Vec<NavItem>,
}

impl Sections {
    pub(super) fn catalog(&self) -> &[NavItem] {
        &self.catalog
    }

    pub(super) fn install(
        &mut self,
        catalog: Vec<NavItem>,
        workspace: &Workspace,
        library: &Library,
    ) {
        self.catalog = catalog;
        self.customize(workspace, library);
    }

    pub(super) fn customize(
        &mut self,
        workspace: &Workspace,
        library: &Library,
    ) {
        let pins = ranks(&workspace.pins);
        let order = ranks(&workspace.navigation_order);
        let mut items = self.catalog.clone();
        items.sort_by_cached_key(|item| {
            let key = identity(&item.view);
            if let Some(rank) = pins.get(&key) {
                (0, *rank)
            } else {
                (1, order.get(&key).copied().unwrap_or(usize::MAX))
            }
        });
        for item in &mut items {
            if pins.contains_key(&identity(&item.view)) {
                item.group = "PINNED";
            }
        }
        self.groups = group_catalog(items, library);
    }

    pub(super) fn project(&self, collapsed: &[String]) -> Vec<NavItem> {
        let mut visible =
            Vec::with_capacity(self.catalog.len() + self.groups.len());
        for group in &self.groups {
            let folded = collapsed.iter().any(|name| name == group.name);
            if group.name != "LIBRARY" {
                let marker = if folded { "▸" } else { "▾" };
                visible.push(NavItem {
                    view: View::Group(group.name.into()),
                    title: format!("{marker} {}", group.name),
                    group: group.name,
                    count: group.count,
                });
            }
            if !folded || group.name == "LIBRARY" {
                visible.extend(group.children.iter().cloned());
            }
        }
        visible
    }

    pub(super) fn destinations(&self) -> Vec<View> {
        self.groups
            .iter()
            .flat_map(|group| {
                group.children.iter().map(|item| item.view.clone())
            })
            .collect()
    }
}

fn ranks(views: &[View]) -> HashMap<View, usize> {
    let mut ranks = HashMap::with_capacity(views.len());
    for (index, view) in views.iter().enumerate() {
        ranks.entry(identity(view)).or_insert(index);
    }
    ranks
}

fn identity(view: &View) -> View {
    match view {
        View::SinceLastVisit(_) => View::SinceLastVisit(0),
        _ => view.clone(),
    }
}

fn group_catalog(items: Vec<NavItem>, library: &Library) -> Vec<Section> {
    let roots = items
        .iter()
        .filter_map(|item| match item.view {
            View::Server(id) if item.group == "SERVERS" => Some(id),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let covered = library
        .feeds
        .iter()
        .filter_map(|feed| {
            feed.server
                .as_ref()
                .filter(|remote| roots.contains(&remote.server_id))
                .map(|_| feed.id)
        })
        .collect::<HashSet<_>>();
    let mut positions = HashMap::new();
    let mut groups = Vec::<Section>::new();
    for item in items {
        let index = *positions.entry(item.group).or_insert_with(|| {
            groups.push(Section {
                name: item.group,
                count: 0,
                children: Vec::new(),
            });
            groups.len() - 1
        });
        let covered_by_server = item.group == "SERVERS"
            && matches!(item.view, View::Feed(id) if covered.contains(&id));
        if !covered_by_server {
            groups[index].count += item.count;
        }
        groups[index].children.push(item);
    }
    groups
}
