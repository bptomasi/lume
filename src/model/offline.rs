//! Captured offline plans keep article identity, progress and resource bounds.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Explicit per-preparation storage and article budgets.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfflineLimits {
    /// Maximum captured articles, between one and one hundred.
    pub articles: usize,
    /// Maximum logical package bytes, between 1 and 512 MiB.
    pub bytes: u64,
}

impl Default for OfflineLimits {
    fn default() -> Self {
        Self {
            articles: 100,
            bytes: 64 * 1024 * 1024,
        }
    }
}

impl OfflineLimits {
    /// Validate bounded work before admitting a preparation job.
    ///
    /// # Errors
    /// Returns invalid article or byte budgets.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=100).contains(&self.articles),
            "Offline article limit is 1–100"
        );
        ensure!(
            (1_048_576..=536_870_912).contains(&self.bytes),
            "Offline byte limit is 1–512 MiB"
        );
        Ok(())
    }
}

/// Outcome of preparing a captured article; partial never claims full availability.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineStatus {
    /// No preparation attempt has completed.
    Pending,
    /// Full readable text and every discovered image were cached.
    Ready,
    /// Feed text or some images are available; inspect the error.
    Partial,
    /// No readable cached source was available.
    Failed,
}

/// One immutable article target and its latest preparation outcome.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfflineItem {
    /// Durable article identity captured before downloading.
    pub article: i64,
    /// Original URL; a changed publisher target is never overwritten.
    pub url: String,
    /// Bounded, terminal-safe headline.
    pub title: String,
    /// Cached availability.
    pub status: OfflineStatus,
    /// Estimated logical package bytes, including images.
    pub bytes: u64,
    /// Number of images cached for this item.
    pub images: usize,
    /// Bounded extraction, image or cancellation explanation.
    pub error: String,
}

/// Durable latest package; incomplete entries can be resumed explicitly.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OfflineState {
    /// Captured destination name.
    pub name: String,
    /// Matching entries before the article limit was applied.
    pub total: usize,
    /// Storage and work limits retained for resume.
    pub limits: OfflineLimits,
    /// Captured targets in reading order.
    pub items: Vec<OfflineItem>,
    /// Whether the last worker had not yet finished or was interrupted.
    pub running: bool,
    /// User cancellation remains distinct from a completed package.
    pub cancelled: bool,
}

impl OfflineState {
    /// Reject corrupted or oversized persisted state before resuming it.
    ///
    /// # Errors
    /// Returns invalid budgets, oversized strings or inconsistent identities.
    pub fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        ensure!(
            self.items.len() <= self.limits.articles,
            "Offline plan is too large"
        );
        ensure!(self.name.len() <= 240, "Offline name exceeds its limit");
        ensure!(
            self.items
                .iter()
                .map(|item| item.bytes)
                .try_fold(0_u64, u64::checked_add)
                .is_some_and(|bytes| bytes <= self.limits.bytes),
            "Offline checkpoint exceeds its byte budget"
        );
        let mut seen = std::collections::BTreeSet::new();
        for item in &self.items {
            ensure!(
                item.article > 0 && seen.insert(item.article),
                "Duplicate or invalid offline target"
            );
            ensure!(
                item.url.len() <= 8192
                    && item.title.len() <= 1000
                    && item.error.len() <= 2048,
                "Offline target exceeds text limits"
            );
        }
        Ok(())
    }
}
