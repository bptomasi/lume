//! Previewed policies, bounded transactions and compact reimport suppression.
//!
//! A plan owns temporary tables on this connection. Its data_version is captured
//! before a read snapshot. External commits interrupt a pass before another batch;
//! the next pass rebuilds protections. Personal data is rechecked in each batch.

mod apply;
mod explain;
pub(crate) use apply::Step;
mod plan;
#[cfg(test)]
mod tests;

use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension};
use sha2::Digest;

use crate::{config::retention::Retention, store::Store};

pub(crate) struct Plan {
    pub(crate) policy: Retention,
    pub(crate) fingerprint: String,
    pub(crate) version: i64,
    pub(crate) upper: i64,
}

impl Store {
    /// Materialize a retention preset without deleting content or arming cleanup.
    ///
    /// # Errors
    /// Returns invalid stored configuration or atomic SQLite failures.
    pub fn set_retention_preset(
        &self,
        preset: crate::config::retention::presets::Preset,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        let mut policy = self.retention_policy()?;
        preset.apply(&mut policy);
        self.save_retention_policy(&policy)?;
        transaction.commit()?;
        Ok(())
    }

    /// Read the configured retention policy without scanning articles.
    ///
    /// # Errors
    /// Returns invalid stored policy or SQLite errors.
    pub fn retention_policy(&self) -> Result<Retention> {
        let value: Option<String> = self
            .connection
            .prepare_cached(include_str!("sql/retention/select_policy.sql"))?
            .query_row([], |row| row.get(0))
            .optional()?;
        let policy: Retention = value
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
        policy.validate()?;
        Ok(policy)
    }

    pub(super) fn save_retention_policy(
        &self,
        policy: &Retention,
    ) -> Result<()> {
        policy.validate()?;
        if self.retention_policy()? != *policy {
            self.connection
                .prepare_cached(include_str!(
                    "sql/retention/update_policy.sql"
                ))?
                .execute([serde_json::to_string(policy)?])?;
        }
        Ok(())
    }

    pub(super) fn forget_retention_smart_list(&self, id: i64) -> Result<()> {
        let mut policy = self.retention_policy()?;
        if policy.unprotected_smart_lists.is_empty() {
            return Ok(());
        }
        let key = self.retention_definition_key("smart", id)?;
        policy
            .unprotected_smart_lists
            .retain(|item| Some(item) != key.as_ref());
        self.save_retention_policy(&policy)
    }

    pub(super) fn ensure_retention_server_unreferenced(
        &self,
        id: i64,
    ) -> Result<()> {
        let policy = self.retention_policy()?;
        if policy.sources.is_empty() {
            return Ok(());
        }
        if let Some(key) = self.retention_definition_key("server", id)? {
            let prefix = format!("server:{key}:");
            ensure!(
                !policy
                    .sources
                    .iter()
                    .any(|source| source.id.starts_with(&prefix)),
                "Server has retention overrides; remove them or pause the server first"
            );
        }
        Ok(())
    }

    fn retention_definition_key(
        &self,
        kind: &str,
        id: i64,
    ) -> Result<Option<String>> {
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "sql/retention/select_definition_key.sql"
            ))?
            .query_row((kind, id), |row| row.get(0))
            .optional()?)
    }

    pub(crate) fn retention_state(&self, key: &str) -> Result<Option<String>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/retention/select_state.sql"))?
            .query_row([key], |row| row.get(0))
            .optional()?)
    }

    pub(crate) fn record_retention_state(
        &self,
        key: &str,
        value: &str,
    ) -> Result<()> {
        self.connection
            .prepare_cached(include_str!("sql/retention/update_state.sql"))?
            .execute((key, value))?;
        Ok(())
    }

    pub(crate) fn retention_version(&self) -> Result<i64> {
        Ok(self.connection.query_row(
            include_str!("sql/retention/select_data_version.sql"),
            [],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn retention_approved(
        &self,
        policy: &Retention,
    ) -> Result<bool> {
        Ok(self.retention_state("retention_preview")?.as_deref()
            == Some(&fingerprint(policy)?))
    }

    pub(crate) fn retention_current(&self, plan: &Plan) -> Result<bool> {
        Ok(self.retention_version()? == plan.version)
    }

    pub(crate) fn require_retention_preview(
        &self,
        policy: &Retention,
    ) -> Result<()> {
        ensure!(
            self.retention_approved(policy)?,
            "Retention policy has not been previewed; run lume retention preview first"
        );
        Ok(())
    }

    pub(crate) fn visit_retention_images(
        &self,
        mut visit: impl FnMut(&str, i64) -> Result<()>,
    ) -> Result<()> {
        let sql = protected_query(include_str!(
            "sql/retention/select_image_urls.sql"
        ));
        let mut statement = self.connection.prepare_cached(&sql)?;
        let mut rows = statement.query([])?;
        while let Some(row) = rows.next()? {
            let url: String = row.get(0)?;
            let cutoff: i64 = row.get(1)?;
            visit(&url, cutoff)?;
        }
        Ok(())
    }

    pub(crate) fn retention_image_batch(
        &self,
        plan: &Plan,
        operation: impl FnOnce() -> Result<()>,
    ) -> Result<bool> {
        let transaction = self.write_transaction()?;
        if !self.retention_current(plan)? {
            return Ok(false);
        }
        operation()?;
        transaction.commit()?;
        Ok(true)
    }
}

pub(crate) fn fingerprint(policy: &Retention) -> Result<String> {
    let scope = (
        &policy.defaults,
        &policy.sources,
        policy.protect_smart_lists,
        &policy.unprotected_smart_lists,
    );
    Ok(sha2::Sha256::digest(serde_json::to_vec(&scope)?)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub(super) fn removed(
    connection: &Connection,
    feed: i64,
    external: &str,
    canonical: &str,
) -> Result<bool> {
    Ok(connection
        .prepare_cached(include_str!("sql/retention/select_removed.sql"))?
        .query_row((feed, external, canonical), |row| row.get(0))?)
}

fn protected_query(tail: &str) -> String {
    let protected = include_str!("sql/retention/select_protected.sql")
        .trim()
        .trim_end_matches(';');
    format!("WITH protected AS MATERIALIZED ({protected})\n{tail}")
}
