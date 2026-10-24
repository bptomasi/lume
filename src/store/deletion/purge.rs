//! Explicit irreversible cleanup shares one transaction with its source change.

use std::collections::BTreeSet;

use anyhow::Result;
use rusqlite::Transaction;

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
        let deleted = transaction.query_row(
            include_str!("../sql/deletion/select_deleted.sql"),
            [article],
            |row| row.get::<_, bool>(0),
        )?;
        anyhow::ensure!(
            deleted,
            "Restore changed this article; deletion canceled"
        );
        prepare(&transaction)?;
        transaction.execute(
            include_str!("../sql/deletion/insert_trash.sql"),
            [Some(article)],
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
        prepare(&transaction)?;
        transaction.execute(
            include_str!("../sql/deletion/insert_trash.sql"),
            [None::<i64>],
        )?;
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
    let articles = identities(
        transaction,
        include_str!("../sql/deletion/select_articles.sql"),
    )?;
    if articles.is_empty() {
        return Ok(0);
    }
    transaction
        .execute_batch(include_str!("../sql/deletion/select_notes.sql"))?;
    let notes = identities(
        transaction,
        include_str!("../sql/deletion/select_note_ids.sql"),
    )?;
    history::forget_articles(transaction, &articles, &notes)?;
    transaction.execute_batch(include_str!(
        "../sql/deletion/update_saved_targets.sql"
    ))?;
    transaction
        .execute_batch(include_str!("../sql/deletion/insert_tombstones.sql"))?;
    transaction
        .execute_batch(include_str!("../sql/deletion/delete_dependents.sql"))?;
    Ok(articles.len())
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
        .replace(
            "{protections}",
            include_str!("../sql/retention/select_protected.sql")
                .trim()
                .trim_end_matches(';'),
        )
        .replace("{sources}", scope);
    transaction.execute(&sql, [id])?;
    remove(transaction)
}

fn identities(
    transaction: &Transaction<'_>,
    sql: &str,
) -> Result<BTreeSet<i64>> {
    Ok(transaction
        .prepare_cached(sql)?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}
