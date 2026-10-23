//! Automatic snapshots are opt-in and run only while Lume is active.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Local snapshot scheduling and owned rotation policy.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Backups {
    /// Enable automatic backups while the TUI runs, and after CLI sync.
    pub automatic: bool,
    /// Minimum elapsed hours after a successfully published backup.
    pub interval_hours: u32,
    /// Number of complete automatic bundles to retain, including the newest.
    pub keep: u16,
    /// Empty uses the library's backups directory; custom paths must be absolute.
    pub directory: PathBuf,
    /// Include bounded pinned offline image originals; ordinary cache is omitted.
    pub include_offline: bool,
}
impl Default for Backups {
    fn default() -> Self {
        Self {
            automatic: false,
            interval_hours: 24,
            keep: 7,
            directory: PathBuf::new(),
            include_offline: false,
        }
    }
}
impl Backups {
    /// Validate work/rotation bounds before any filesystem operation.
    ///
    /// # Errors
    /// Returns invalid intervals, rotation counts or custom paths.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            (1..=8760).contains(&self.interval_hours),
            "Backup interval must be 1–8760 hours"
        );
        ensure!(
            (1..=365).contains(&self.keep),
            "Backup rotation must retain 1–365 bundles"
        );
        ensure!(
            self.directory.as_os_str().is_empty()
                || self.directory.is_absolute(),
            "Backup directory must be absolute or empty"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn backup_preferences_round_trip_and_reject_unbounded_or_relative_settings()
    -> anyhow::Result<()> {
        let config = crate::config::decode(
            "[backups]\nautomatic=true\ninterval_hours=6\nkeep=3\ninclude_offline=true\ndirectory='/tmp/snapshots'",
        )?;
        assert_eq!(
            crate::config::decode(&crate::config::encode(&config)?)?,
            config
        );
        for text in [
            "interval_hours=0",
            "interval_hours=8761",
            "keep=0",
            "keep=366",
            "directory='relative'",
        ] {
            assert!(
                crate::config::decode(&format!("[backups]\n{text}")).is_err()
            );
        }
        Ok(())
    }
}
