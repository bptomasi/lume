//! Snapshot scheduling is durable; declarative policy changes preserve history.
use super::Store;
use crate::config::backups::Backups;
use anyhow::Result;
use rusqlite::OptionalExtension;

impl Store {
    /// Read local snapshot preferences without scanning the article library.
    ///
    /// # Errors
    /// Returns invalid persisted policy or SQLite failures.
    pub fn backup_policy(&self) -> Result<Backups> {
        let value: Option<String> = self
            .connection
            .query_row(
                include_str!("sql/backups/select_policy.sql"),
                [],
                |row| row.get(0),
            )
            .optional()?;
        let policy: Backups = value
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
        policy.validate()?;
        Ok(policy)
    }

    pub(super) fn save_backup_policy(&self, policy: &Backups) -> Result<()> {
        policy.validate()?;
        if self.backup_policy()? != *policy {
            self.connection.execute(
                include_str!("sql/backups/update_policy.sql"),
                [serde_json::to_string(policy)?],
            )?;
        }
        Ok(())
    }

    pub(crate) fn backup_last_run(&self) -> Result<Option<i64>> {
        Ok(self
            .connection
            .query_row(
                include_str!("sql/backups/select_last_run.sql"),
                [],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(crate) fn record_backup_run(&self, now: i64) -> Result<()> {
        self.connection
            .execute(include_str!("sql/backups/update_last_run.sql"), [now])?;
        Ok(())
    }
}
