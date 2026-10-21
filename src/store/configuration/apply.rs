//! Reconciliation updates definitions by stable binding, preserving personal rows.

use std::collections::BTreeSet;

use anyhow::Result;

use crate::{
    config::{Configuration, Feed, Folder, SmartList, Ui},
    model::{PanelLayout, RecommendationProfile, SmartFilter},
    store::Store,
};

impl Store {
    pub(super) fn apply_configuration(
        &self,
        config: &Configuration,
        previous: &Configuration,
    ) -> Result<()> {
        config.validate()?;
        for feed in
            changed_definitions(&previous.feeds, &config.feeds, |item| &item.id)
        {
            self.apply_configuration_feed(feed)?;
        }
        for folder in
            changed_definitions(&previous.folders, &config.folders, |item| {
                &item.id
            })
        {
            self.apply_configuration_folder(folder)?;
        }
        for list in changed_definitions(
            &previous.smart_lists,
            &config.smart_lists,
            |item| &item.id,
        ) {
            self.apply_configuration_smart(list)?;
        }
        for list in changed_definitions(
            &previous.note_collections,
            &config.note_collections,
            |item| &item.id,
        ) {
            self.apply_configuration_note_collection(list)?;
        }
        self.archive_configuration(config)?;
        self.apply_configuration_preferences(config)
    }

    fn apply_configuration_feed(&self, feed: &Feed) -> Result<()> {
        let url = crate::source::feed_url(&feed.url)?;
        let id = if let Some(id) = self.configuration_id("feed", &feed.id)? {
            self.connection.execute(
                include_str!("../sql/configuration/update_feed.sql"),
                (id, url.as_str(), feed.enabled, &feed.selector),
            )?;
            id
        } else {
            self.connection.execute(
                include_str!("../sql/configuration/insert_feed.sql"),
                (url.as_str(), feed.enabled, &feed.selector),
            )?;
            self.connection.query_row(
                "SELECT id FROM feeds WHERE url = ?1",
                [url.as_str()],
                |row| row.get(0),
            )?
        };
        self.bind_configuration("feed", id, &feed.id)
    }

    fn apply_configuration_folder(&self, folder: &Folder) -> Result<()> {
        let id =
            if let Some(id) = self.configuration_id("folder", &folder.id)? {
                self.connection.execute(
                    include_str!("../sql/configuration/update_folder.sql"),
                    (id, &folder.name),
                )?;
                id
            } else {
                self.connection.execute(
                    include_str!("../sql/configuration/insert_folder.sql"),
                    [&folder.name],
                )?;
                self.connection.query_row(
                    "SELECT id FROM collections WHERE name = ?1",
                    [&folder.name],
                    |row| row.get(0),
                )?
            };
        self.bind_configuration("folder", id, &folder.id)
    }

    fn apply_configuration_smart(&self, list: &SmartList) -> Result<()> {
        let filter = SmartFilter {
            text: list.query.clone(),
            unread: list.unread,
            starred: list.starred,
            feed: list
                .feed
                .as_ref()
                .map(|key| self.required_feed(key))
                .transpose()?,
            feeds: list
                .feeds
                .iter()
                .map(|key| self.required_feed(key))
                .collect::<Result<_>>()?,
            days: list.days,
        };
        let json = serde_json::to_string(&filter)?;
        let id = if let Some(id) = self.configuration_id("smart", &list.id)? {
            self.connection.execute(
                include_str!("../sql/configuration/update_smart_list.sql"),
                (id, &list.name, &json),
            )?;
            id
        } else {
            self.connection.execute(
                include_str!("../sql/configuration/insert_smart_list.sql"),
                (&list.name, json),
            )?;
            self.connection.query_row(
                "SELECT id FROM smart_lists WHERE name = ?1",
                [&list.name],
                |row| row.get(0),
            )?
        };
        self.bind_configuration("smart", id, &list.id)
    }

    fn required_feed(&self, key: &str) -> Result<i64> {
        self.configuration_id("feed", key)?
            .ok_or_else(|| anyhow::anyhow!("Unknown feed reference: {key}"))
    }

    fn apply_configuration_note_collection(&self, list: &Folder) -> Result<()> {
        let id = if let Some(id) = self.configuration_id("notes", &list.id)? {
            self.connection.execute(
                include_str!("../sql/configuration/update_note_collection.sql"),
                (id, &list.name, &list.query),
            )?;
            id
        } else {
            self.connection.execute(
                include_str!("../sql/configuration/insert_note_collection.sql"),
                (&list.name, &list.query),
            )?;
            self.connection.query_row(
                "SELECT id FROM note_collections WHERE name = ?1",
                [&list.name],
                |row| row.get(0),
            )?
        };
        self.bind_configuration("notes", id, &list.id)
    }

    fn archive_configuration(&self, config: &Configuration) -> Result<()> {
        let wanted = definition_keys(config);
        let rows = self
            .connection
            .prepare_cached(include_str!(
                "../sql/configuration/select_bindings.sql"
            ))?
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (kind, id, key) in rows {
            if wanted.contains(&(kind.as_str(), key.as_str())) {
                continue;
            }
            let sql = match kind.as_str() {
                "feed" => "UPDATE feeds SET enabled = 0 WHERE id = ?1",
                "folder" => "UPDATE collections SET enabled = 0 WHERE id = ?1",
                "smart" => "UPDATE smart_lists SET enabled = 0 WHERE id = ?1",
                "notes" => {
                    "UPDATE note_collections SET enabled = 0 WHERE id = ?1"
                }
                _ => anyhow::bail!("Unknown configuration binding kind"),
            };
            self.connection.execute(sql, [id])?;
            self.connection.execute(
                r#"
                UPDATE configuration_bindings SET present = 0
                WHERE kind = ?1 AND entity = ?2
                "#,
                (kind, id),
            )?;
        }
        Ok(())
    }

    fn apply_configuration_preferences(
        &self,
        config: &Configuration,
    ) -> Result<()> {
        let sources = config
            .feeds
            .iter()
            .filter(|feed| feed.priority != 0)
            .map(|feed| Ok((self.required_feed(&feed.id)?, feed.priority)))
            .collect::<Result<_>>()?;
        let profile = RecommendationProfile {
            interests: config.recommendations.interests.clone(),
            exclude: config.recommendations.exclude.clone(),
            minutes: config.recommendations.minutes,
            include_read: config.recommendations.include_read,
            sources,
        };
        crate::recommendation::validate_profile(&profile)?;
        if profile != self.recommendation_profile()? {
            self.replace_recommendation_profile(&profile)?;
        }
        let mut workspace = self.workspace()?;
        let mut layouts = self.layouts(&workspace)?;
        if config.ui == Ui::capture(&workspace)
            && config.workspaces == layouts.profiles
        {
            return Ok(());
        }
        config.ui.apply(&mut workspace);
        super::super::workspace::validate_dimensions(&workspace)?;
        layouts.profiles = config.workspaces.clone();
        layouts.observe(PanelLayout::capture(&workspace));
        self.save_workspace(&workspace)?;
        self.connection.execute(
            include_str!("../sql/insert_layout_history.sql"),
            [serde_json::to_string(&layouts)?],
        )?;
        Ok(())
    }
}

fn changed_definitions<'a, T: PartialEq>(
    previous: &'a [T],
    current: &'a [T],
    key: impl Fn(&T) -> &str + 'a,
) -> impl Iterator<Item = &'a T> {
    // Configuration normalizes all four definition collections by stable ID.
    // Binary lookup avoids quadratic work for large subscription files.
    current.iter().filter(move |item| {
        previous
            .binary_search_by(|before| key(before).cmp(key(item)))
            .ok()
            .is_none_or(|index| &previous[index] != *item)
    })
}

fn definition_keys(config: &Configuration) -> BTreeSet<(&str, &str)> {
    config
        .feeds
        .iter()
        .map(|item| ("feed", item.id.as_str()))
        .chain(
            config
                .folders
                .iter()
                .map(|item| ("folder", item.id.as_str())),
        )
        .chain(
            config
                .smart_lists
                .iter()
                .map(|item| ("smart", item.id.as_str())),
        )
        .chain(
            config
                .note_collections
                .iter()
                .map(|item| ("notes", item.id.as_str())),
        )
        .collect()
}
