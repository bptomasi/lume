//! Offline checkpoints are independent of publisher refreshes and personal undo.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use super::Store;
use crate::model::OfflineState;

impl Store {
    pub(super) fn prepare_offline_restore(
        &self,
        preserve_images: bool,
    ) -> Result<()> {
        let Some(mut state) = self.offline_state()? else {
            return Ok(());
        };
        let mut changed = state.running;
        state.running = false;
        for item in &mut state.items {
            let bytes: Option<u64> = self
                .connection
                .query_row(
                    include_str!("sql/select_restored_offline_bytes.sql"),
                    (item.article, &item.url),
                    |row| row.get(0),
                )
                .optional()?;
            if bytes.is_some() && (preserve_images || item.images == 0) {
                continue;
            }
            item.status = if bytes.is_some() {
                crate::model::OfflineStatus::Partial
            } else {
                crate::model::OfflineStatus::Failed
            };
            item.bytes = bytes.unwrap_or(0).min(item.bytes);
            item.images = 0;
            item.error = if bytes.is_some() {
                "Offline images were omitted from this snapshot; use :offline resume to prepare them again."
            } else {
                "Offline target is unavailable in this snapshot; capture a new package."
            }.into();
            changed = true;
        }
        if changed {
            self.save_offline_state(&state)?;
        }
        Ok(())
    }

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
