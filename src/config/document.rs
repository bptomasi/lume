//! Patch the parsed TOML tree rather than reserializing the user's whole file.

use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table, TableLike};

use super::Configuration;

/// Reuse exact user bytes when definitions did not change; otherwise patch
/// against the already validated input rather than decoding it a second time.
pub(crate) fn render(
    text: Option<&str>,
    previous: Option<&Configuration>,
    config: &Configuration,
) -> Result<String> {
    match (text, previous) {
        (Some(text), Some(previous)) if previous == config => Ok(text.into()),
        (Some(text), Some(previous)) => patch(text, previous, config),
        (None, None) => patch(
            "# Lume configuration. Personal reading data stays in SQLite.\nschema_version = 1\n",
            &Configuration::default(),
            config,
        ),
        _ => anyhow::bail!("Configuration text and parsed input diverged"),
    }
}

pub(crate) fn patch(
    text: &str,
    previous: &Configuration,
    config: &Configuration,
) -> Result<String> {
    let mut current = text.parse::<DocumentMut>()?;
    let previous = toml_edit::ser::to_document(previous)?;
    let desired = toml_edit::ser::to_document(config)?;
    patch_table(
        current.as_table_mut(),
        previous.as_table(),
        desired.as_table(),
    );
    Ok(current.to_string())
}

fn patch_table(
    current: &mut dyn TableLike,
    previous: &dyn TableLike,
    desired: &dyn TableLike,
) {
    let removed = previous
        .iter()
        .filter(|(key, _)| !desired.contains_key(key))
        .map(|(key, _)| key.to_owned())
        .collect::<Vec<_>>();
    for key in removed {
        current.remove(&key);
    }
    for (key, desired) in desired.iter() {
        let before = previous.get(key);
        if before
            .is_some_and(|before| before.to_string() == desired.to_string())
        {
            continue;
        }
        if let Some(existing) = current.get_mut(key) {
            patch_item(existing, before, desired);
        } else {
            current.insert(key, desired.clone());
        }
    }
}

fn patch_item(current: &mut Item, previous: Option<&Item>, desired: &Item) {
    if let Some(desired) = desired.as_table_like()
        && let Some(current) = current.as_table_like_mut()
    {
        patch_table(
            current,
            previous
                .and_then(Item::as_table_like)
                .unwrap_or(&Table::new()),
            desired,
        );
        return;
    }
    match (current, desired) {
        (Item::ArrayOfTables(current), Item::ArrayOfTables(desired)) => {
            patch_records(
                current,
                previous.and_then(Item::as_array_of_tables),
                desired,
            );
        }
        (Item::Value(current), Item::Value(desired)) => {
            let decor = current.decor().clone();
            *current = desired.clone();
            *current.decor_mut() = decor;
        }
        (current, desired) => *current = desired.clone(),
    }
}

fn identity(table: &Table) -> Option<&str> {
    table.get("id").or_else(|| table.get("name"))?.as_str()
}

fn patch_records(
    current: &mut ArrayOfTables,
    previous: Option<&ArrayOfTables>,
    desired: &ArrayOfTables,
) {
    let desired_keys =
        desired.iter().filter_map(identity).collect::<BTreeSet<_>>();
    current.retain(|item| {
        identity(item).is_some_and(|id| desired_keys.contains(id))
    });
    let positions = current
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            identity(item).map(|id| (id.to_owned(), index))
        })
        .collect::<BTreeMap<_, _>>();
    let previous = previous
        .into_iter()
        .flat_map(|tables| tables.iter())
        .filter_map(|item| identity(item).map(|id| (id, item)))
        .collect::<BTreeMap<_, _>>();
    for item in desired {
        let id = identity(item).unwrap_or("");
        if let Some(existing) =
            positions.get(id).and_then(|index| current.get_mut(*index))
        {
            patch_table(
                existing,
                previous.get(id).copied().unwrap_or(&Table::new()),
                item,
            );
        } else {
            current.push(item.clone());
        }
    }
}
