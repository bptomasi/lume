//! Portable identities survive publication replay and domain row renames.

use anyhow::Result;
use rusqlite::OptionalExtension;

use crate::{config::Configuration, store::Store};

pub(super) enum Adoption {
    Initial,
    Unbound,
}

impl Store {
    pub(super) fn configuration_id(
        &self,
        kind: &str,
        key: &str,
    ) -> Result<Option<i64>> {
        Ok(self
            .connection
            .query_row(
                include_str!("../sql/configuration/select_binding_entity.sql"),
                (kind, key),
                |row| row.get(0),
            )
            .optional()?)
    }

    pub(super) fn bind_configuration(
        &self,
        kind: &str,
        id: i64,
        key: &str,
    ) -> Result<()> {
        self.connection.execute(
            include_str!("../sql/configuration/insert_binding.sql"),
            (kind, id, key),
        )?;
        Ok(())
    }

    pub(super) fn configuration_key(
        &self,
        kind: &str,
        id: i64,
    ) -> Result<String> {
        Ok(self.connection.query_row(
            include_str!("../sql/configuration/select_binding_key.sql"),
            (kind, id),
            |row| row.get(0),
        )?)
    }

    pub(super) fn adopt_configuration_keys(
        &self,
        config: &Configuration,
        adoption: Adoption,
    ) -> Result<()> {
        for server in &config.servers {
            let url = crate::urls::server_url(&server.url)?;
            let id = self
                .connection
                .query_row(
                    include_str!("../sql/servers/select_id.sql"),
                    [url.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if let Some(id) = id {
                self.adopt_binding("server", id, &server.id, &adoption)?;
            }
        }
        for feed in &config.feeds {
            let url = crate::urls::feed_url(&feed.url)?;
            let id = self
                .connection
                .query_row(
                    include_str!("../sql/configuration/select_feed_id.sql"),
                    [url.as_str()],
                    |row| row.get::<_, i64>(0),
                )
                .optional()?;
            if let Some(id) = id {
                self.adopt_binding("feed", id, &feed.id, &adoption)?;
            }
        }
        for folder in &config.folders {
            self.adopt_named("folder", &folder.id, &folder.name, &adoption)?;
        }
        for list in &config.smart_lists {
            self.adopt_named("smart", &list.id, &list.name, &adoption)?;
        }
        for list in &config.note_collections {
            self.adopt_named("notes", &list.id, &list.name, &adoption)?;
        }
        Ok(())
    }

    fn adopt_named(
        &self,
        kind: &str,
        key: &str,
        name: &str,
        adoption: &Adoption,
    ) -> Result<()> {
        let sql = match kind {
            "folder" => {
                include_str!("../sql/configuration/select_folder_id.sql")
            }
            "smart" => include_str!("../sql/configuration/select_smart_id.sql"),
            "notes" => include_str!(
                "../sql/configuration/select_note_collection_id.sql"
            ),
            _ => return Ok(()),
        };
        let id = self
            .connection
            .query_row(sql, [name], |row| row.get::<_, i64>(0))
            .optional()?;
        if let Some(id) = id {
            self.adopt_binding(kind, id, key, adoption)?;
        }
        Ok(())
    }

    fn adopt_binding(
        &self,
        kind: &str,
        id: i64,
        key: &str,
        adoption: &Adoption,
    ) -> Result<()> {
        let bound: bool = self.connection.query_row(
            include_str!("../sql/configuration/select_binding_exists.sql"),
            (kind, id),
            |row| row.get(0),
        )?;
        if !bound || matches!(adoption, Adoption::Initial) {
            self.bind_configuration(kind, id, key)?;
        }
        Ok(())
    }
}
