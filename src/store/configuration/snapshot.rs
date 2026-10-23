//! Capture only declarative definitions, excluding personal reading state.

use anyhow::Result;

use crate::{
    config::{self, Configuration, Recommendations, Ui},
    model::SmartFilter,
    store::Store,
};

impl Store {
    pub(in crate::store) fn capture_configuration(
        &self,
    ) -> Result<Configuration> {
        self.connection.execute_batch(include_str!(
            "../sql/configuration/insert_bindings.sql"
        ))?;
        if self.has_table("server_feeds")? {
            self.connection.execute_batch(include_str!(
                "../sql/configuration/delete_server_feed_bindings.sql"
            ))?;
            self.connection.execute_batch(include_str!(
                "../sql/servers/insert_bindings.sql"
            ))?;
        }
        let (workspace, layouts) = self.configuration_preferences()?;
        let profile = self.recommendation_profile()?;
        let feeds = self.configuration_feeds(&profile)?;
        let mut config = Configuration {
            schema_version: 1,
            ui: Ui::capture(&workspace),
            feeds,
            servers: self.configuration_servers()?,
            folders: self.configuration_folders()?,
            smart_lists: self.configuration_lists()?,
            note_collections: self.configuration_note_collections()?,
            recommendations: Recommendations {
                interests: profile.interests,
                exclude: profile.exclude,
                minutes: profile.minutes,
                include_read: profile.include_read,
            },
            workspaces: layouts.profiles,
            retention: self.retention_policy()?,
            backups: self.backup_policy()?,
        };
        config.normalize();
        config.validate()?;
        Ok(config)
    }

    fn configuration_feeds(
        &self,
        profile: &crate::model::RecommendationProfile,
    ) -> Result<Vec<config::Feed>> {
        self.connection
            .prepare_cached(include_str!(
                "../sql/configuration/select_feeds.sql"
            ))?
            .query_map([], |row| {
                let id: i64 = row.get(0)?;
                Ok(config::Feed {
                    id: row.get(1)?,
                    url: row.get(2)?,
                    enabled: row.get(3)?,
                    selector: row.get(4)?,
                    fetch_policy: match row.get::<_, String>(5)?.as_str() {
                        "missing" => crate::model::FetchPolicy::Missing,
                        "always" => crate::model::FetchPolicy::Always,
                        "manual" => crate::model::FetchPolicy::Manual,
                        _ => return Err(rusqlite::Error::InvalidQuery),
                    },
                    priority: profile.sources.get(&id).copied().unwrap_or(0),
                })
            })?
            .collect::<rusqlite::Result<_>>()
            .map_err(Into::into)
    }

    fn configuration_servers(&self) -> Result<Vec<config::Server>> {
        if !self.has_table("servers")? {
            return Ok(Vec::new());
        }
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "../sql/configuration/select_servers.sql"
            ))?
            .query_map([], |row| {
                Ok(config::Server {
                    id: row.get(1)?,
                    url: row.get(2)?,
                    token_env: row.get(3)?,
                    token: row
                        .get::<_, Option<String>>(5)?
                        .filter(|token| !token.is_empty())
                        .map(Into::into),
                    enabled: row.get(4)?,
                    sync: super::super::ingest::server_sync_row(row, 6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn has_table(&self, name: &str) -> Result<bool> {
        Ok(self.connection.query_row(
            r#"
            SELECT EXISTS (
                SELECT 1 FROM sqlite_master
                WHERE type = 'table' AND name = ?1
            )
            "#,
            [name],
            |row| row.get(0),
        )?)
    }

    fn configuration_preferences(
        &self,
    ) -> Result<(crate::model::Workspace, crate::model::LayoutHistory)> {
        let workspace = self.workspace()?;
        let layouts = self.layouts(&workspace)?;
        // Seed defaults inside reconciliation. Later progress-only saves must
        // compare against these rows, not appear to insert new definitions.
        self.connection.execute(
            include_str!("../sql/configuration/insert_initial_preferences.sql"),
            (
                serde_json::to_string(&workspace)?,
                serde_json::to_string(&layouts)?,
            ),
        )?;
        Ok((workspace, layouts))
    }

    fn configuration_folders(&self) -> Result<Vec<config::Folder>> {
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "../sql/configuration/select_folders.sql"
            ))?
            .query_map([], |row| {
                Ok(config::Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    query: String::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn configuration_note_collections(&self) -> Result<Vec<config::Folder>> {
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "../sql/configuration/select_note_collections.sql"
            ))?
            .query_map([], |row| {
                Ok(config::Folder {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    query: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn configuration_lists(&self) -> Result<Vec<config::SmartList>> {
        let rows = self
            .connection
            .prepare_cached(include_str!(
                "../sql/configuration/select_smart_lists.sql"
            ))?
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(id, name, json)| {
                let filter: SmartFilter = serde_json::from_str(&json)?;
                Ok(config::SmartList {
                    id,
                    name,
                    query: filter.text,
                    unread: filter.unread,
                    starred: filter.starred,
                    feed: filter
                        .feed
                        .map(|id| self.configuration_feed_key(id))
                        .transpose()?,
                    feeds: filter
                        .feeds
                        .into_iter()
                        .map(|id| self.configuration_feed_key(id))
                        .collect::<Result<_>>()?,
                    days: filter.days,
                })
            })
            .collect()
    }
}
