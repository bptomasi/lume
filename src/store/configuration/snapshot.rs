//! Capture only declarative definitions, excluding personal reading state.

use anyhow::Result;

use crate::{
    config::{self, Configuration, Recommendations, Ui},
    model::SmartFilter,
    store::Store,
};

impl Store {
    pub(super) fn capture_configuration(&self) -> Result<Configuration> {
        self.connection.execute_batch(include_str!(
            "../sql/configuration/insert_bindings.sql"
        ))?;
        let (workspace, layouts) = self.configuration_preferences()?;
        let profile = self.recommendation_profile()?;
        let feeds = self
            .connection
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
                    priority: profile.sources.get(&id).copied().unwrap_or(0),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        let mut config = Configuration {
            schema_version: 1,
            ui: Ui::capture(&workspace),
            feeds,
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
        };
        config.normalize();
        config.validate()?;
        Ok(config)
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
                        .map(|id| self.configuration_key("feed", id))
                        .transpose()?,
                    feeds: filter
                        .feeds
                        .into_iter()
                        .map(|id| self.configuration_key("feed", id))
                        .collect::<Result<_>>()?,
                    days: filter.days,
                })
            })
            .collect()
    }
}
