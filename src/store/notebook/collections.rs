//! Named note queries and their explicit submission history remain bounded.

use crate::{model::NoteCollection, store::Store};
use anyhow::{Result, ensure};

impl Store {
    /// Read bounded user-authored dynamic note collections.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn note_collections(&self) -> Result<Vec<NoteCollection>> {
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "../sql/notebook/select_collections.sql"
            ))?
            .query_map([], |row| {
                Ok(NoteCollection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    query: row.get(2)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Save or update a named query; its syntax is identical to live note search.
    ///
    /// # Errors
    /// Rejects invalid names/queries, a 100-collection limit or SQLite errors.
    pub fn save_note_collection(&self, name: &str, query: &str) -> Result<()> {
        let name = super::super::annotations::collection_name(name)?;
        crate::search::notes::validate(query)?;
        let transaction = self.write_transaction()?;
        let count: usize = transaction.query_row(
            r#"
            SELECT count(*)
            FROM note_collections
            WHERE enabled = 1 AND name <> ?1 COLLATE NOCASE
            "#,
            [name],
            |row| row.get(0),
        )?;
        ensure!(count < 100, "At most 100 note collections");
        transaction.execute(
            include_str!("../sql/notebook/insert_collection.sql"),
            (name, query),
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Delete a user-owned collection by its exact name, retaining all notes.
    ///
    /// # Errors
    /// Returns a missing collection or SQLite failure.
    pub fn delete_note_collection(&self, name: &str) -> Result<()> {
        let changed = self.connection.execute(
            "DELETE FROM note_collections WHERE name = ?1 COLLATE NOCASE",
            [name],
        )?;
        ensure!(changed == 1, "No saved note collection has this name");
        Ok(())
    }

    /// Return the latest 30 successful explicit searches.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn note_search_history(&self) -> Result<Vec<String>> {
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "../sql/notebook/select_search_history.sql"
            ))?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Remember an explicitly submitted query, independently of per-key search.
    ///
    /// # Errors
    /// Rejects invalid queries or SQLite errors.
    pub fn remember_note_search(&self, query: &str) -> Result<()> {
        crate::search::notes::validate(query)?;
        if query.trim().is_empty() {
            return Ok(());
        }
        let transaction = self.write_transaction()?;
        transaction.execute(
            include_str!("../sql/notebook/insert_search_history.sql"),
            [query],
        )?;
        transaction.execute_batch(include_str!(
            "../sql/notebook/delete_old_search_history.sql"
        ))?;
        transaction.commit()?;
        Ok(())
    }
}
