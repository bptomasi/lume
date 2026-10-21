//! Three-way structural merge: unrelated edits compose; conflicts keep both inputs.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};
use serde_json::Value;

use super::Configuration;

pub(crate) fn configuration(
    base: &Configuration,
    local: &Configuration,
    file: &Configuration,
) -> Result<Configuration> {
    let merged = merge(
        Some(&serde_json::to_value(base)?),
        Some(&serde_json::to_value(local)?),
        Some(&serde_json::to_value(file)?),
        "configuration",
    )?;
    let mut config: Configuration =
        serde_json::from_value(merged.unwrap_or(Value::Null))?;
    config.normalize();
    config.validate()?;
    Ok(config)
}

fn merge(
    base: Option<&Value>,
    local: Option<&Value>,
    file: Option<&Value>,
    path: &str,
) -> Result<Option<Value>> {
    if local == base || local == file {
        return Ok(file.cloned());
    }
    if file == base {
        return Ok(local.cloned());
    }
    match (base, local, file) {
        (base, Some(Value::Object(local)), Some(Value::Object(file))) => {
            let base = base.and_then(Value::as_object);
            let keys = local
                .keys()
                .chain(file.keys())
                .chain(base.into_iter().flat_map(|map| map.keys()))
                .collect::<BTreeSet<_>>();
            let mut merged = serde_json::Map::new();
            for key in keys {
                if let Some(value) = merge(
                    base.and_then(|map| map.get(key)),
                    local.get(key),
                    file.get(key),
                    &format!("{path}.{key}"),
                )? {
                    merged.insert(key.clone(), value);
                }
            }
            Ok(Some(Value::Object(merged)))
        }
        (base, Some(Value::Array(local)), Some(Value::Array(file))) => {
            merge_records(base.and_then(Value::as_array), local, file, path)
        }
        _ => bail!(
            "Configuration conflict at {path}; keep the file and pending \
            library edits, then resolve with `lume config recover file|library`"
        ),
    }
}

fn merge_records(
    base: Option<&Vec<Value>>,
    local: &[Value],
    file: &[Value],
    path: &str,
) -> Result<Option<Value>> {
    let empty = Vec::new();
    let base = base.unwrap_or(&empty);
    let sample = base
        .first()
        .or_else(|| local.first())
        .or_else(|| file.first());
    let key = if sample.and_then(|item| item.get("id")).is_some() {
        "id"
    } else {
        "name"
    };
    let (Some(base), Some(local), Some(file)) =
        (keyed(base, key), keyed(local, key), keyed(file, key))
    else {
        bail!("Configuration conflict at {path}");
    };
    let keys = base
        .keys()
        .chain(local.keys())
        .chain(file.keys())
        .collect::<BTreeSet<_>>();
    let mut records = Vec::new();
    for key in keys {
        if let Some(value) = merge(
            base.get(key).copied(),
            local.get(key).copied(),
            file.get(key).copied(),
            &format!("{path}[{key}]"),
        )? {
            records.push(value);
        }
    }
    Ok(Some(Value::Array(records)))
}

fn keyed<'a>(
    items: &'a [Value],
    key: &str,
) -> Option<BTreeMap<&'a str, &'a Value>> {
    let mut map = BTreeMap::new();
    for item in items {
        let id = item.get(key)?.as_str()?;
        if map.insert(id, item).is_some() {
            return None;
        }
    }
    Some(map)
}
