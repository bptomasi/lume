//! Atomic personal-list membership changes and reversible list lifecycle.

use std::collections::BTreeSet;

use anyhow::Result;
use rusqlite::{OptionalExtension, Transaction};

use crate::model::Collection;

use super::{
    Store,
    history::{self, Undo},
};

impl Store {
    /// Read only the memberships of one article for the list selector.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn article_memberships(&self, article: i64) -> Result<BTreeSet<i64>> {
        Ok(self
            .connection
            .prepare_cached(
                "SELECT collection_id FROM members WHERE article_id = ?1",
            )?
            .query_map([article], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Create or locate a list, comparing names case-insensitively.
    ///
    /// # Errors
    /// Returns invalid name or SQLite errors.
    pub fn create_collection(&self, name: &str) -> Result<i64> {
        self.save_collection(name, None)
    }

    /// Create a folder and optionally add one article in the same undo action.
    ///
    /// # Errors
    /// Returns invalid names, missing articles or SQLite transaction failures.
    pub fn save_collection(
        &self,
        name: &str,
        article: Option<i64>,
    ) -> Result<i64> {
        let transaction = self.write_transaction()?;
        let (id, mut undo) = collection(&transaction, name)?;
        if let Some(article) = article {
            undo.push(membership_snapshot(&transaction, article)?);
            transaction.execute(
                include_str!("sql/insert_list_member.sql"),
                (id, article),
            )?;
        }
        if !undo.is_empty() {
            history::record(&transaction, &Undo::Batch(undo))?;
        }
        transaction.commit()?;
        Ok(id)
    }

    /// Replace captured memberships and create named folders as one transaction.
    ///
    /// # Errors
    /// Returns invalid names/owners or SQLite errors without partial changes.
    pub fn set_named_memberships(
        &self,
        article: i64,
        folders: &BTreeSet<i64>,
        names: &BTreeSet<String>,
    ) -> Result<()> {
        anyhow::ensure!(
            folders.len() + names.len() <= 1000,
            "At most 1,000 folders"
        );
        let transaction = self.write_transaction()?;
        let mut lists = folders.clone();
        let mut undo = Vec::new();
        for name in names {
            let (id, created) = collection(&transaction, name)?;
            lists.insert(id);
            undo.extend(created);
        }
        undo.push(membership_snapshot(&transaction, article)?);
        replace_memberships(
            &transaction,
            article,
            &lists.into_iter().collect::<Vec<_>>(),
        )?;
        history::record(&transaction, &Undo::Batch(undo))?;
        transaction.commit()?;
        Ok(())
    }

    /// Rename a list, preserving membership and recording its previous state.
    ///
    /// # Errors
    /// Returns invalid/duplicate name, missing list or SQLite errors.
    pub fn rename_collection(&self, id: i64, name: &str) -> Result<()> {
        let name = super::annotations::collection_name(name)?;
        let transaction = self.write_transaction()?;
        history::record(
            &transaction,
            &Undo::Collection(snapshot(&transaction, id)?),
        )?;
        transaction.execute(
            "UPDATE collections SET name = ?1 WHERE id = ?2",
            (name, id),
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Delete a list while retaining articles and a reversible snapshot.
    ///
    /// # Errors
    /// Returns missing list or SQLite errors.
    pub fn delete_collection(&self, id: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        history::record(
            &transaction,
            &Undo::Collection(snapshot(&transaction, id)?),
        )?;
        transaction.execute("DELETE FROM collections WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(())
    }

    /// Replace all of an article's memberships as one reversible action.
    ///
    /// # Errors
    /// Returns missing identities or SQLite errors; all changes roll back.
    pub fn set_memberships(
        &self,
        article: i64,
        lists: &BTreeSet<i64>,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        record_memberships(&transaction, article)?;
        replace_memberships(
            &transaction,
            article,
            &lists.iter().copied().collect::<Vec<_>>(),
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Add a membership without replacing any other list.
    ///
    /// # Errors
    /// Returns missing identities or SQLite errors.
    pub fn add_member(&self, collection: i64, article: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        record_memberships(&transaction, article)?;
        transaction.execute(
            include_str!("sql/insert_list_member.sql"),
            (collection, article),
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Remove membership without deleting article state.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn remove_member(&self, collection: i64, article: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        record_memberships(&transaction, article)?;
        transaction.execute(
            "DELETE FROM members WHERE collection_id = ?1 AND article_id = ?2",
            (collection, article),
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn record_memberships(
    transaction: &Transaction<'_>,
    article: i64,
) -> Result<()> {
    history::record(transaction, &membership_snapshot(transaction, article)?)
}

fn membership_snapshot(
    transaction: &Transaction<'_>,
    article: i64,
) -> Result<Undo> {
    let lists = transaction
        .prepare_cached(
            "SELECT collection_id FROM members WHERE article_id = ?1",
        )?
        .query_map([article], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Undo::Memberships { article, lists })
}

fn collection(
    transaction: &Transaction<'_>,
    name: &str,
) -> Result<(i64, Vec<Undo>)> {
    let name = super::annotations::collection_name(name)?;
    let existing = transaction
        .query_row(
            "SELECT id FROM collections WHERE name = ?1",
            [name],
            |row| row.get::<_, i64>(0),
        )
        .optional()?;
    if let Some(id) = existing {
        transaction.execute(
            "UPDATE collections SET enabled = 1 WHERE id = ?1",
            [id],
        )?;
        return Ok((id, Vec::new()));
    }
    transaction
        .execute(include_str!("sql/configuration/insert_folder.sql"), [name])?;
    let id = transaction.last_insert_rowid();
    Ok((id, vec![Undo::DeleteCollection(id)]))
}

pub(super) fn replace_memberships(
    transaction: &Transaction<'_>,
    article: i64,
    lists: &[i64],
) -> Result<()> {
    transaction.query_row(
        "SELECT id FROM articles WHERE id = ?1",
        [article],
        |_| Ok(()),
    )?;
    transaction
        .execute("DELETE FROM members WHERE article_id = ?1", [article])?;
    for list in lists {
        transaction.execute(
            "INSERT INTO members (collection_id, article_id) VALUES (?1, ?2)",
            (list, article),
        )?;
    }
    Ok(())
}

fn snapshot(transaction: &Transaction<'_>, id: i64) -> Result<Collection> {
    let name = transaction.query_row(
        "SELECT name FROM collections WHERE id = ?1",
        [id],
        |row| row.get(0),
    )?;
    let articles = transaction
        .prepare_cached(
            "SELECT article_id FROM members WHERE collection_id = ?1",
        )?
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(Collection { id, name, articles })
}
