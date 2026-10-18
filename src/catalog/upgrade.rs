//! Upgrade bundled labels without rewriting user criteria or active identities.
//!
//! Only exact historical names belong to this upgrade. Destination conflicts
//! retain both lists using an English imported suffix. Renames commit together;
//! repeated installation sees no historical labels and creates no duplicates.

use std::collections::BTreeSet;

use anyhow::{Result, bail};

use crate::{
    model::{SmartList, SmartListRename},
    store::Store,
};

use super::{SECURITY_LIST, SavedSearch};

pub(super) fn english_labels(
    store: &Store,
    searches: &[SavedSearch],
) -> Result<()> {
    let names = planned_renames(store.smart_lists()?, searches)?;
    if !names.is_empty() {
        store.rename_smart_lists(&names)?;
    }
    store.rename_feed_title_if_current(
        "https://blog.acolyer.org/feed/",
        "The Morning Paper - acervo",
        "The Morning Paper — Archive",
    )?;
    Ok(())
}

fn planned_renames(
    lists: Vec<SmartList>,
    searches: &[SavedSearch],
) -> Result<Vec<SmartListRename>> {
    let mut used = lists
        .iter()
        .map(|list| list.name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let replacements = searches
        .iter()
        .filter_map(|search| {
            search
                .previous_name
                .as_deref()
                .map(|old| (old, search.name.as_str()))
        })
        .chain(std::iter::once((
            "19 · Segurança computacional",
            SECURITY_LIST,
        )))
        .collect::<Vec<_>>();
    let mut names = Vec::new();
    for list in lists {
        let Some((_, replacement)) =
            replacements.iter().find(|(old, _)| *old == list.name)
        else {
            continue;
        };
        let name = available_name(replacement, &used)?;
        used.remove(&list.name.to_ascii_lowercase());
        used.insert(name.to_ascii_lowercase());
        names.push(SmartListRename {
            id: list.id,
            expected_name: list.name,
            name,
        });
    }
    Ok(names)
}

fn available_name(name: &str, used: &BTreeSet<String>) -> Result<String> {
    if !used.contains(&name.to_ascii_lowercase()) {
        return Ok(name.to_owned());
    }
    for number in 1..=used.len().saturating_add(1) {
        let candidate = if number == 1 {
            format!("{name} (imported)")
        } else {
            format!("{name} (imported {number})")
        };
        if !used.contains(&candidate.to_ascii_lowercase()) {
            return Ok(candidate);
        }
    }
    bail!("Could not reserve an English name for {name}")
}
