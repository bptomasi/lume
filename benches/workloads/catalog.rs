//! Catalog upgrade timing excludes database and legacy-label setup.

use std::time::{Duration, Instant};

use anyhow::{Result, ensure};
use lume::{
    catalog,
    model::{SmartFilter, SmartListRename},
    store::Store,
};

const LEGACY: &str = "19 · Segurança computacional";
const IMPORTED: &str = "19 · Security (imported)";

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("catalog.db"))?;
    let custom = if stress { 1000 } else { 40 };
    let filter = prepare(&store, custom)?;
    let legacy = store
        .smart_lists()?
        .into_iter()
        .find(|list| list.name == LEGACY)
        .ok_or_else(|| anyhow::anyhow!("Missing legacy fixture"))?;
    let mut elapsed = Duration::ZERO;
    for iteration in 0..iterations {
        if iteration > 0 {
            store.rename_smart_lists(&[SmartListRename {
                id: legacy.id,
                expected_name: IMPORTED.into(),
                name: LEGACY.into(),
            }])?;
        }
        let started = Instant::now();
        let result = catalog::install(&store)?;
        elapsed += started.elapsed();
        ensure!(result.added_feeds == 0, "Subscription duplication");
        ensure!(result.added_searches == 0, "Query duplication");
        verify(&store, legacy.id, custom, &filter)?;
    }
    println!(
        "catalog English upgrade: 98 sources, 46 topics, {custom} custom \
         queries; {:.1} µs/cycle ({iterations} cycles)",
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64
    );
    source_groups(&store, iterations)?;
    Ok(())
}

fn source_groups(store: &Store, iterations: usize) -> Result<()> {
    let names = [
        catalog::PLATFORM_LIST,
        catalog::KERNEL_LIST,
        catalog::NETWORK_LIST,
        catalog::DEVOPS_LIST,
    ];
    let mut elapsed = Duration::ZERO;
    for _ in 0..iterations {
        for list in store.smart_lists()? {
            if names.contains(&list.name.as_str()) {
                store.delete_smart_list(list.id)?;
            }
        }
        let started = Instant::now();
        let result = catalog::install(store)?;
        elapsed += started.elapsed();
        ensure!(result.added_feeds == 0, "Subscriptions duplicated");
        ensure!(result.added_searches == 4, "Source groups missing");
        verify_groups(store)?;
    }
    println!(
        "install four infrastructure groups: {:.1} µs/cycle ({iterations} cycles)",
        elapsed.as_secs_f64() * 1_000_000.0 / iterations as f64
    );
    Ok(())
}

fn verify_groups(store: &Store) -> Result<()> {
    let lists = store.smart_lists()?;
    let sources = store.feeds()?;
    for (name, count, representative) in [
        (catalog::PLATFORM_LIST, 8, "Backstage"),
        (catalog::KERNEL_LIST, 7, "Bootlin"),
        (catalog::NETWORK_LIST, 8, "Cilium"),
        (catalog::DEVOPS_LIST, 8, "Tekton"),
    ] {
        let list = lists
            .iter()
            .find(|list| list.name == name)
            .ok_or_else(|| anyhow::anyhow!("Missing group {name}"))?;
        let source =
            sources
                .iter()
                .find(|feed| feed.title == representative)
                .ok_or_else(|| anyhow::anyhow!("Missing {representative}"))?;
        ensure!(list.filter.feeds.len() == count, "Wrong group size");
        ensure!(
            list.filter.feeds.contains(&source.id),
            "Source membership lost"
        );
    }
    Ok(())
}

fn prepare(store: &Store, custom: usize) -> Result<SmartFilter> {
    catalog::install(store)?;
    let filter = SmartFilter {
        text: "Research criteria retained".into(),
        unread: true,
        starred: true,
        days: Some(90),
        ..SmartFilter::default()
    };
    for index in 0..custom {
        store
            .save_smart_list(&format!("Personal query {index:04}"), &filter)?;
    }
    store.save_smart_list(LEGACY, &filter)?;
    Ok(filter)
}

fn verify(
    store: &Store,
    identity: i64,
    custom: usize,
    filter: &SmartFilter,
) -> Result<()> {
    let lists = store.smart_lists()?;
    ensure!(lists.len() == custom + 47, "Saved query lost or duplicated");
    let upgraded = lists
        .iter()
        .find(|list| list.id == identity)
        .ok_or_else(|| anyhow::anyhow!("Saved identity changed"))?;
    ensure!(upgraded.name == IMPORTED, "Conflict label was not retained");
    ensure!(upgraded.filter == *filter, "Personal criteria overwritten");
    for list in lists
        .iter()
        .filter(|s| s.name.starts_with("Personal query"))
    {
        ensure!(list.filter == *filter, "Unrelated criteria changed");
    }
    ensure!(store.feeds()?.len() == 98, "Subscription selection lost");
    Ok(())
}
