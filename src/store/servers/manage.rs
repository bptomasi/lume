//! Connection edits retain saved personal context and stable portable identity.

use anyhow::{Result, ensure};

use crate::store::Store;

impl Store {
    /// Restart initial-history discovery without deleting cached or personal data.
    ///
    /// # Errors
    /// Returns missing server or SQLite failures; subsequent refreshes resume it.
    pub fn restart_server_history(&self, id: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        ensure!(
            self.servers()?.iter().any(|server| server.id == id),
            "Unknown server"
        );
        transaction.execute(
            include_str!("../sql/servers/restart_history.sql"),
            [id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Add or reconnect a Miniflux server using an external token variable.
    ///
    /// # Errors
    /// Rejects invalid URLs, token variable names, limits or SQLite failures.
    pub fn add_server(&self, address: &str, token_env: &str) -> Result<i64> {
        crate::config::validate_env_name(token_env)?;
        self.persist_server(address, token_env, "")
    }

    /// Add or reconnect with an explicitly stored TOML API token.
    ///
    /// # Errors
    /// Returns invalid URL/token, server limit or atomic SQLite failures.
    pub fn add_server_token(
        &self,
        address: &str,
        token: &crate::model::ApiToken,
    ) -> Result<i64> {
        token.validate()?;
        self.persist_server(address, "", token.expose())
    }

    fn persist_server(
        &self,
        address: &str,
        token_env: &str,
        token: &str,
    ) -> Result<i64> {
        let url = crate::source::server_url(address)?;
        let transaction = self.write_transaction()?;
        let count: usize = transaction.query_row(
            include_str!("../sql/servers/count_other_servers.sql"),
            [&url],
            |row| row.get(0),
        )?;
        ensure!(count < 16, "At most 16 servers");
        transaction.execute(
            include_str!("../sql/configuration/insert_server.sql"),
            (&url, token_env, true, token),
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

    /// Remove a connection and unsaved articles, preserving personal context.
    ///
    /// # Errors
    /// Rejects missing connections or SQLite failures; changes are atomic.
    pub fn remove_server(&self, id: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        ensure!(
            self.servers()?.iter().any(|server| server.id == id),
            "Server no longer exists"
        );
        self.forget_server_scopes(id)?;
        self.forget_retention_server(id)?;
        super::super::deletion::purge::source(
            &transaction,
            super::super::deletion::purge::Source::Server(id),
        )?;
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

    fn forget_server_scopes(&self, id: i64) -> Result<()> {
        let feeds = self
            .connection
            .prepare_cached(include_str!("../sql/servers/select_feed_ids.sql"))?
            .query_map([id], |row| row.get::<_, i64>(0))?
            .collect::<rusqlite::Result<std::collections::BTreeSet<_>>>()?;
        for list in self.smart_lists()? {
            let mut filter = list.filter.clone();
            let single_removed =
                filter.feed.is_some_and(|id| feeds.contains(&id));
            let group_restricted = !filter.feeds.is_empty();
            filter.feeds.retain(|id| !feeds.contains(id));
            if single_removed {
                filter.feed = None;
            }
            if single_removed || group_restricted && filter.feeds.is_empty() {
                // An emptied source restriction must never become global.
                // Keep the named list editable with an explicit empty query.
                filter.text = if filter.text.is_empty() {
                    "is:read AND is:unread".into()
                } else {
                    format!("({}) AND is:read AND is:unread", filter.text)
                };
            }
            if filter != list.filter {
                self.save_smart_list(&list.name, &filter)?;
            }
        }
        Ok(())
    }
}
