//! Opt-in computing and AI subscriptions and literal topic searches.
//!
//! OPML assets own subscriptions; JSON owns literal search definitions and
//! historical labels. Installation upgrades owned labels in place and keeps
//! existing criteria. Network collection remains an explicit sync or the
//! terminal's normal startup action.

mod groups;
mod upgrade;

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use serde::Deserialize;

use crate::{
    exchange::opml, model::SmartFilter, source::discovery::Candidate,
    store::Store,
};

/// Result of installing the bundled reading selection.
pub struct InstallReport {
    /// Newly added or explicitly re-enabled subscriptions.
    pub added_feeds: usize,
    /// Number of subscriptions in the selection.
    pub total_feeds: usize,
    /// New topic searches; existing criteria are preserved.
    pub added_searches: usize,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedSearch {
    name: String,
    filter: SmartFilter,
    #[serde(default)]
    previous_name: Option<String>,
}

/// Decode and validate the bundled OPML without accessing the network.
///
/// # Errors
/// Returns invalid bundled XML, URL or resource-limit errors.
pub fn feeds() -> Result<Vec<Candidate>> {
    let mut feeds =
        opml::decode(include_str!("../assets/reading/computing.opml"))?;
    feeds.extend(security_feeds()?);
    feeds.extend(ai_feeds()?);
    feeds.extend(infrastructure_feeds()?);
    Ok(feeds)
}

/// Security research subscriptions used by the automatic security list.
///
/// # Errors
/// Returns invalid bundled OPML or subscription URLs.
pub fn security_feeds() -> Result<Vec<Candidate>> {
    opml::decode(include_str!("../assets/reading/security.opml"))
}

/// AI research, model engineering and combined paper subscriptions.
///
/// # Errors
/// Returns invalid bundled OPML or subscription URLs.
pub fn ai_feeds() -> Result<Vec<Candidate>> {
    opml::decode(include_str!("../assets/reading/ai.opml"))
}

/// Platform, kernel and networking subscriptions from the expansion asset.
///
/// # Errors
/// Returns invalid bundled OPML or subscription URLs.
pub fn infrastructure_feeds() -> Result<Vec<Candidate>> {
    opml::decode(include_str!("../assets/reading/infrastructure.opml"))
}

/// Install subscriptions and missing topic searches without replacing notes.
///
/// # Errors
/// Returns bundled-data or SQLite errors. Subscription import is atomic;
/// historical label renames commit as a batch before new searches commit
/// individually. Retry preserves search identities, criteria and article state.
pub fn install(store: &Store) -> Result<InstallReport> {
    if store.recommendation_profile()?.interests.is_empty() {
        store.save_recommendation_profile(&engineering_profile()?)?;
    }
    let feeds = feeds()?;
    let searches: Vec<SavedSearch> =
        serde_json::from_str(include_str!("../assets/reading/searches.json"))?;
    upgrade::english_labels(store, &searches)?;
    let existing = store
        .smart_lists()?
        .into_iter()
        .map(|list| list.name.to_ascii_lowercase())
        .collect::<BTreeSet<_>>();
    let added_feeds = store.import_feeds(&feeds)?;
    let added_searches = install_searches(store, searches, &existing)?
        + groups::install(store, &existing)?;
    Ok(InstallReport {
        added_feeds,
        total_feeds: feeds.len(),
        added_searches,
    })
}

/// Explicit engineering preset interests; never a new library's defaults.
///
/// # Errors
/// Returns invalid bundled profile data.
pub fn engineering_profile() -> Result<crate::model::RecommendationProfile> {
    Ok(crate::model::RecommendationProfile {
        interests: serde_json::from_str(include_str!(
            "../assets/reading/interests.json"
        ))?,
        ..Default::default()
    })
}

fn install_searches(
    store: &Store,
    searches: Vec<SavedSearch>,
    existing: &BTreeSet<String>,
) -> Result<usize> {
    let mut added = 0;
    for search in searches {
        if existing.contains(&search.name.to_ascii_lowercase()) {
            continue;
        }
        store
            .save_smart_list(&search.name, &search.filter)
            .with_context(|| {
                format!(
                    "Subscriptions saved; rerun starter to finish topic {}",
                    search.name
                )
            })?;
        added += 1;
    }
    Ok(added)
}

/// Name of the automatically populated security research list.
pub const SECURITY_LIST: &str = "19 · Security";

/// Name of the automatically populated AI reading list.
pub const AI_LIST: &str = "20 · Artificial intelligence";

/// Name of the automatically populated developer-platform reading list.
pub const PLATFORM_LIST: &str = "31 · Platform engineering";

/// Name of the automatically populated kernel and systems reading list.
pub const KERNEL_LIST: &str = "32 · Kernel and systems";

/// Name of the automatically populated networking reading list.
pub const NETWORK_LIST: &str = "33 · Networks and dataplanes";

/// Name of the automatically populated operations and delivery reading list.
pub const DEVOPS_LIST: &str = "34 · DevOps and delivery";
