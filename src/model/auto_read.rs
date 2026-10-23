//! Configurable reading intent is independent of selection and saved progress.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Automatic read state requires active Reader time and accumulated coverage.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AutoReadSettings {
    /// Enable automatic marking; manual read/unread actions remain available.
    pub enabled: bool,
    /// Active seconds with the Reader focused before marking can occur.
    pub min_active_seconds: u16,
    /// Minimum percentage of readable article text actually displayed.
    pub min_coverage_percent: u16,
}

impl Default for AutoReadSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            min_active_seconds: 10,
            min_coverage_percent: 90,
        }
    }
}

impl AutoReadSettings {
    /// Validate bounded, positive thresholds; disable through `enabled`.
    ///
    /// # Errors
    /// Rejects times outside 1–3,600 seconds or coverage outside 1–100 percent.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=3600).contains(&self.min_active_seconds),
            "Automatic reading time must be 1–3,600 seconds"
        );
        ensure!(
            (1..=100).contains(&self.min_coverage_percent),
            "Automatic reading coverage must be 1–100 percent"
        );
        Ok(())
    }
}
