//! Explicit reading plans retain their order independently of recommendation updates.

use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Captured metadata used to plan a bounded reading session without article bodies.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SessionItem {
    /// Durable article identity.
    pub article: i64,
    /// Publisher identity for diversity.
    pub feed: i64,
    /// Captured address, also used to reject changed targets.
    pub url: String,
    /// Canonical address used for duplicate detection.
    pub canonical: String,
    /// Terminal-safe headline.
    pub title: String,
    /// Estimated reading duration, with a one-minute minimum.
    pub minutes: u16,
    /// Positive cached recommendation score.
    pub score: i64,
    /// Primary matching interest.
    pub topic: String,
}

/// Durable, user-confirmed sequence; advancing never changes reading flags.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReadingSession {
    /// Available minutes, between one and 240.
    pub budget: u16,
    /// Diverse ordered sequence, at most eight articles.
    pub items: Vec<SessionItem>,
    /// Current position; equal to the item count after completion.
    pub position: usize,
}

impl ReadingSession {
    /// Validate persisted or captured plans before they can be used.
    ///
    /// # Errors
    /// Returns invalid identities, sizes or time budgets.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=240).contains(&self.budget),
            "Session budget is 1–240 minutes"
        );
        ensure!(self.items.len() <= 8, "Session has too many articles");
        ensure!(
            self.position <= self.items.len(),
            "Invalid session position"
        );
        let mut seen = std::collections::BTreeSet::new();
        for item in &self.items {
            ensure!(
                item.article > 0 && seen.insert(item.article),
                "Invalid session identity"
            );
            ensure!(
                item.minutes > 0
                    && item.title.len() <= 1000
                    && item.url.len() <= 8192,
                "Invalid session metadata"
            );
        }
        ensure!(
            self.items
                .iter()
                .map(|item| u32::from(item.minutes))
                .sum::<u32>()
                <= u32::from(self.budget),
            "Session exceeds its time budget"
        );
        Ok(())
    }
}
