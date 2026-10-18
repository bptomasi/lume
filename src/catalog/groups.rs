//! Resolve curated source membership to local subscription identities.
//!
//! The asset references canonical OPML URLs, never database-specific IDs.
//! Existing named queries keep their criteria. Missing source references fail
//! explicitly rather than broadening a partially resolved group.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;

use crate::{model::SmartFilter, store::Store};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceGroup {
    name: String,
    sources: Vec<String>,
}

pub(super) fn install(
    store: &Store,
    existing: &BTreeSet<String>,
) -> Result<usize> {
    let groups: Vec<SourceGroup> = serde_json::from_str(include_str!(
        "../../assets/reading/source-groups.json"
    ))?;
    let sources = store
        .feeds()?
        .into_iter()
        .map(|feed| (feed.url, feed.id))
        .collect::<BTreeMap<_, _>>();
    let mut added = 0;
    for group in groups {
        if existing.contains(&group.name.to_ascii_lowercase()) {
            continue;
        }
        store
            .save_smart_list(&group.name, &group.filter(&sources)?)
            .with_context(|| {
                format!(
                    "Subscriptions saved; rerun starter to finish {}",
                    group.name
                )
            })?;
        added += 1;
    }
    Ok(added)
}

impl SourceGroup {
    fn filter(&self, sources: &BTreeMap<String, i64>) -> Result<SmartFilter> {
        ensure!(
            (1..=128).contains(&self.sources.len()),
            "Source group must contain 1–128 references: {}",
            self.name
        );
        let unique = self.sources.iter().collect::<BTreeSet<_>>();
        ensure!(
            unique.len() == self.sources.len(),
            "Duplicate source reference in {}",
            self.name
        );
        let feeds = self
            .sources
            .iter()
            .map(|url| {
                sources.get(url).copied().with_context(|| {
                    format!("Missing source {url} for {}", self.name)
                })
            })
            .collect::<Result<_>>()?;
        Ok(SmartFilter {
            feeds,
            ..SmartFilter::default()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unresolved_groups_never_become_unrestricted_queries() {
        let known = BTreeMap::from([("https://source.example/feed".into(), 7)]);
        for references in [
            Vec::new(),
            vec!["https://missing.example/feed".into()],
            vec![
                "https://source.example/feed".into(),
                "https://missing.example/feed".into(),
            ],
            vec!["https://source.example/feed".into(); 2],
            vec!["https://source.example/feed".into(); 129],
        ] {
            let group = SourceGroup {
                name: "Invalid membership".into(),
                sources: references,
            };
            assert!(group.filter(&known).is_err());
        }
    }
}
