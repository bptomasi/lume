//! Explicit irreversible cleanup shares one transaction with its source change.

use anyhow::Result;
use rusqlite::{OptionalExtension, Transaction};

use crate::store::{Store, history};

impl Store {
    /// Permanently remove one article that is still in Trash.
    ///
    /// Missing articles are an idempotent no-op; restored articles are rejected.
    /// Removes attached personal data and related undo records atomically.
    /// Compact publisher identities prevent the next refresh from reimporting it.
    ///
    /// # Errors
    /// Returns ownership, stored-history or SQLite errors without partial cleanup.
    pub fn purge_deleted(&self, article: i64) -> Result<bool> {
        let transaction = self.write_transaction()?;
        let deleted = transaction
            .query_row(
                include_str!("../sql/deletion/select_deleted.sql"),
                [article],
                |row| row.get::<_, bool>(0),
            )
            .optional()?;
        let Some(deleted) = deleted else {
            return Ok(false);
        };
        anyhow::ensure!(
            deleted,
            "Restore changed this article; deletion canceled"
        );
        prepare(&transaction)?;
        transaction.execute(
            include_str!("../sql/deletion/insert_single.sql"),
            [article],
        )?;
        let count = remove(&transaction)?;
        transaction.commit()?;
        Ok(count != 0)
    }

    /// Permanently empty Trash, including attached notes and related undo data.
    ///
    /// # Errors
    /// Returns stored-history or SQLite errors; the whole cleanup rolls back.
    pub fn clean_trash(&self) -> Result<usize> {
        let transaction = self.write_transaction()?;
        let populated = transaction.query_row(
            include_str!("../sql/deletion/select_has_trash.sql"),
            [],
            |row| row.get::<_, bool>(0),
        )?;
        if !populated {
            return Ok(0);
        }
        prepare(&transaction)?;
        transaction
            .execute(include_str!("../sql/deletion/insert_trash.sql"), [])?;
        let count = remove(&transaction)?;
        transaction.commit()?;
        Ok(count)
    }
}

pub(in crate::store) fn prepare(transaction: &Transaction<'_>) -> Result<()> {
    transaction.execute_batch(include_str!("../sql/deletion/prepare.sql"))?;
    Ok(())
}

pub(in crate::store) fn remove(transaction: &Transaction<'_>) -> Result<usize> {
    let count: usize = transaction.query_row(
        include_str!("../sql/deletion/select_count.sql"),
        [],
        |row| row.get(0),
    )?;
    if count == 0 {
        return Ok(0);
    }
    transaction
        .execute_batch(include_str!("../sql/deletion/select_notes.sql"))?;
    history::forget_articles(transaction)?;
    transaction.execute_batch(include_str!(
        "../sql/deletion/update_saved_targets.sql"
    ))?;
    transaction
        .execute_batch(include_str!("../sql/deletion/insert_tombstones.sql"))?;
    transaction
        .execute_batch(include_str!("../sql/deletion/delete_dependents.sql"))?;
    Ok(count)
}

pub(in crate::store) enum Source {
    Feed(i64),
    Server(i64),
}

pub(in crate::store) fn source(
    transaction: &Transaction<'_>,
    source: Source,
) -> Result<usize> {
    let (id, scope) = match source {
        Source::Feed(id) => {
            (id, include_str!("../sql/deletion/select_feed_scope.sql"))
        }
        Source::Server(id) => {
            (id, include_str!("../sql/deletion/select_server_scope.sql"))
        }
    };
    prepare(transaction)?;
    // Smart queries and undo history are not explicit article saves.
    // Preserve durable personal context independently of retention settings.
    let sql = include_str!("../sql/deletion/insert_source.sql")
        .replace("{sources}", scope);
    transaction.execute(&sql, [id])?;
    remove(transaction)
}
