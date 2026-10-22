//! Offline checkpoints are independent of publisher refreshes and personal undo.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use super::Store;
use crate::model::OfflineState;

impl Store {
    /// Read the latest package without changing its interrupted/running state.
    ///
    /// # Errors
    /// Returns storage or invalid persisted checkpoint errors.
    pub fn offline_state(&self) -> Result<Option<OfflineState>> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'offline_package'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        value
            .map(|value| {
                ensure!(
                    value.len() <= 2_097_152,
                    "Offline checkpoint exceeds 2 MiB"
                );
                let state: OfflineState = serde_json::from_str(&value)?;
                state.validate()?;
                Ok(state)
            })
            .transpose()
    }

    /// Persist one validated checkpoint atomically; no reading flags are changed.
    ///
    /// # Errors
    /// Returns invalid checkpoints or SQLite failures.
    pub fn save_offline_state(&self, state: &OfflineState) -> Result<()> {
        state.validate()?;
        let value = serde_json::to_string(state)?;
        ensure!(value.len() <= 2_097_152, "Offline checkpoint exceeds 2 MiB");
        self.connection.execute(
            r#"
            INSERT INTO settings (key, value)
            VALUES ('offline_package', ?1)
            ON CONFLICT (key) DO UPDATE SET value = excluded.value
            "#,
            [value],
        )?;
        Ok(())
    }

    /// Release package protection; ordinary cached article text remains available.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn release_offline_state(&self) -> Result<()> {
        self.connection.execute(
            "DELETE FROM settings WHERE key = 'offline_package'",
            [],
        )?;
        Ok(())
    }
}
