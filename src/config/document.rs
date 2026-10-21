//! Patch the parsed TOML tree rather than reserializing the user's whole file.

use anyhow::Result;
use std::collections::{BTreeMap, BTreeSet};
use toml_edit::{ArrayOfTables, DocumentMut, Item, Table};

use super::Configuration;

pub(crate) fn patch(text: &str, config: &Configuration) -> Result<String> {
    let mut current = text.parse::<DocumentMut>()?;
    let previous = toml_edit::ser::to_document(&super::decode(text)?)?;
    let desired = toml_edit::ser::to_document(config)?;
    patch_table(
        current.as_table_mut(),
        previous.as_table(),
        desired.as_table(),
    );
    Ok(current.to_string())
}

fn patch_table(current: &mut Table, previous: &Table, desired: &Table) {
    current.retain(|key, _| {
        !previous.contains_key(key) || desired.contains_key(key)
    });
    for (key, desired) in desired {
        let before = previous.get(key);
        if before
            .is_some_and(|before| before.to_string() == desired.to_string())
        {
            continue;
        }
        let Some(existing) = current.get_mut(key) else {
            current.insert(key, desired.clone());
            continue;
        };
        match (existing, desired) {
            (Item::Table(current), Item::Table(desired)) => patch_table(
                current,
                before.and_then(Item::as_table).unwrap_or(&Table::new()),
                desired,
            ),
            (Item::ArrayOfTables(current), Item::ArrayOfTables(desired)) => {
                patch_records(
                    current,
                    before.and_then(Item::as_array_of_tables),
                    desired,
                );
            }
            (Item::Value(current), Item::Value(desired)) => {
                if current.to_string().trim() != desired.to_string().trim() {
                    let decor = current.decor().clone();
                    *current = desired.clone();
                    *current.decor_mut() = decor;
                }
            }
            (current, desired) => *current = desired.clone(),
        }
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
