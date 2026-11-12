//! Bounded first-history import and durable identity cursors for remote feeds.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Per-server import limits; zero initial entries explicitly requests all history.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerSync {
    /// Maximum first-import entries per feed; zero imports all available history.
    pub initial_entries: u32,
    /// Entries in each API response, bounded to avoid oversized JSON allocations.
    pub entries_per_page: u16,
    /// Pages fetched per feed per refresh; completed sets commit atomically.
    pub pages_per_sync: u8,
}
impl Default for ServerSync {
    fn default() -> Self {
        Self {
            initial_entries: 2000,
            entries_per_page: 200,
            pages_per_sync: 10,
        }
    }
}
impl ServerSync {
    /// Reject impractical or unbounded single-refresh allocations.
    ///
    /// # Errors
    /// Returns invalid entry/page bounds.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.initial_entries <= 10_000_000,
            "Initial history exceeds 10 million entries"
        );
        ensure!(
            (1..=200).contains(&self.entries_per_page),
            "Entries per page must be 1–200"
        );
        ensure!(
            (1..=10).contains(&self.pages_per_sync),
            "Pages per sync must be 1–10"
        );
        Ok(())
    }
}
/// Completed initial-import observations, independent of incremental watermarks.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerHistory {
    /// Last visited descending entry identity; zero before the first page.
    pub before_id: i64,
    /// Entries visited, including remotely removed records.
    pub loaded: u64,
    /// Highest observed available total during this import.
    pub total: u64,
}

impl super::ServerFeed {
    /// Describe the durable checkpoint; historical imports resume on refresh.
    pub fn sync_progress(&self) -> String {
        let target = if self.sync.initial_entries == 0 {
            self.history.total
        } else if self.history.total == 0 {
            u64::from(self.sync.initial_entries)
        } else {
            self.history.total.min(u64::from(self.sync.initial_entries))
        };
        format!(
            "History {} · {} / {} entries · {} entries/page, {} pages/refresh",
            if self.initialized {
                "complete"
            } else {
                "pending; refresh resumes"
            },
            self.history.loaded,
            if target == 0 {
                "unknown".into()
            } else {
                target.to_string()
            },
            self.sync.entries_per_page,
            self.sync.pages_per_sync
        )
    }
}
