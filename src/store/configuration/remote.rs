//! Portable remote scopes retain feed identity before subscription discovery.

use anyhow::{Context, Result};
use rusqlite::OptionalExtension;

use crate::store::Store;

impl Store {
    pub(super) fn configuration_feed_key(&self, id: i64) -> Result<String> {
        let remote: Option<(i64, i64)> = self
            .connection
            .query_row(
                include_str!("../sql/configuration/select_remote_scope.sql"),
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((server, remote)) = remote {
            let key = self.configuration_key("server", server)?;
            return Ok(format!("server:{key}:{remote}"));
        }
        self.configuration_key("feed", id)
    }

    pub(super) fn configuration_remote_feed(
        &self,
        key: &str,
        remote: i64,
    ) -> Result<i64> {
        let server = self
            .configuration_id("server", key)?
            .with_context(|| format!("Unknown server reference: {key}"))?;
        if let Some(id) = self
            .connection
            .query_row(
                include_str!("../sql/servers/select_feed_identity.sql"),
                (server, remote),
                |row| row.get(0),
            )
            .optional()?
        {
            return Ok(id);
        }
        self.insert_remote_scope(server, remote)
    }

    fn insert_remote_scope(&self, server: i64, remote: i64) -> Result<i64> {
        let root: String = self.connection.query_row(
            include_str!("../sql/servers/select_url.sql"),
            [server],
            |row| row.get(0),
        )?;
        let url = format!("{root}/v1/feeds/{remote}");
        self.connection.execute(
            include_str!("../sql/upsert_server_feed.sql"),
            (&url, &url, server),
        )?;
        let id = self.connection.last_insert_rowid();
        // This is a scope placeholder, never an implicit subscription. Only a
        // successful discovery may make it visible and eligible for collection.
        self.connection.execute(
            include_str!("../sql/configuration/insert_remote_scope.sql"),
            (server, remote, id),
        )?;
        Ok(id)
    }
}
