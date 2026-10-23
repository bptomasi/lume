//! Connection edits retain cached feeds, articles and stable portable identity.

use anyhow::{Result, ensure};

use crate::store::Store;

impl Store {
    /// Add or reconnect a Miniflux server using an external token variable.
    ///
    /// # Errors
    /// Rejects invalid URLs, token variable names, limits or SQLite failures.
    pub fn add_server(&self, address: &str, token_env: &str) -> Result<i64> {
        let url = crate::source::server_url(address)?;
        crate::config::validate_env_name(token_env)?;
        let transaction = self.write_transaction()?;
        let count: usize = transaction.query_row(
            include_str!("../sql/servers/count_other_servers.sql"),
            [&url],
            |row| row.get(0),
        )?;
        ensure!(count < 16, "At most 16 servers");
        transaction.execute(
            include_str!("../sql/configuration/insert_server.sql"),
            (&url, token_env, true, ""),
        )?;
        let id = transaction.query_row(
            include_str!("../sql/servers/select_id.sql"),
            [&url],
            |row| row.get(0),
        )?;
        transaction.execute_batch(include_str!(
            "../sql/servers/insert_bindings.sql"
        ))?;
        transaction.execute(
            include_str!("../sql/servers/restore_binding.sql"),
            [id],
        )?;
        transaction.execute(
            include_str!("../sql/servers/update_enabled.sql"),
            (id, true),
        )?;
        transaction.execute(
            include_str!("../sql/servers/reset_feed_health.sql"),
            [id],
        )?;
        transaction.commit()?;
        Ok(id)
    }

    /// Pause or resume collection while retaining the visible connection.
    ///
    /// # Errors
    /// Returns missing connection or SQLite failures.
    pub fn set_server_enabled(&self, id: i64, enabled: bool) -> Result<()> {
        let transaction = self.write_transaction()?;
        ensure!(
            self.servers()?.iter().any(|server| server.id == id),
            "Server no longer exists"
        );
        transaction.execute(
            include_str!("../sql/servers/update_enabled.sql"),
            (id, enabled),
        )?;
        if enabled {
            transaction.execute(
                include_str!("../sql/servers/reset_feed_health.sql"),
                [id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Archive the definition without deleting cached or personal data.
    ///
    /// # Errors
    /// Rejects missing connections, smart-list/retention references or
    /// SQLite failures; changes are atomic.
    pub fn remove_server(&self, id: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        ensure!(
            self.servers()?.iter().any(|server| server.id == id),
            "Server no longer exists"
        );
        self.ensure_server_unreferenced(id)?;
        self.ensure_retention_server_unreferenced(id)?;
        transaction.execute_batch(include_str!(
            "../sql/servers/insert_bindings.sql"
        ))?;
        transaction.execute(
            include_str!("../sql/servers/update_enabled.sql"),
            (id, false),
        )?;
        transaction.execute(
            include_str!("../sql/servers/archive_binding.sql"),
            [id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn ensure_server_unreferenced(&self, id: i64) -> Result<()> {
        let feeds = self
            .connection
            .prepare_cached(include_str!("../sql/servers/select_feed_ids.sql"))?
            .query_map([id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
        for list in self.smart_lists()? {
            ensure!(
                !list
                    .filter
                    .feed
                    .iter()
                    .chain(&list.filter.feeds)
                    .any(|feed| feeds.contains(feed)),
                "Server is used by smart list '{}'; pause it or remove its source restriction first",
                list.name
            );
        }
        Ok(())
    }
}
