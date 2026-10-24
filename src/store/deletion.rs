//! Reversible deletion retains publisher identities and all personal state.
//!
//! A deleted article remains as a tombstone across publisher refreshes. Query
//! scope owns its visibility; restoring clears only that flag. History and the
//! visibility change commit together, and no-op requests consume no undo entry.

use anyhow::{Result, ensure};
use rusqlite::Transaction;

pub(super) mod purge;
#[cfg(test)]
mod tests;

use super::{
    Store,
    history::{self, Undo},
};

impl Store {
    /// Move an entire built-in view to Trash as one reversible transaction.
    /// No page limit, interactive search, body loading or preliminary count applies.
    ///
    /// # Errors
    /// Rejects non-library views or SQLite/history failures without partial edits.
    pub fn trash_view(&self, view: &crate::model::View) -> Result<usize> {
        ensure!(view.can_trash_all(), "Select a built-in Library view");
        let transaction = self.write_transaction()?;
        let articles = self.trash_matching(view)?;
        let count = articles.len();
        if count != 0 {
            history::record(&transaction, &Undo::DeletionBatch { articles })?;
        }
        transaction.commit()?;
        Ok(count)
    }

    /// Read an article's durable deletion flag, including hidden articles.
    ///
    /// # Errors
    /// Returns missing-article or SQLite errors.
    pub fn is_deleted(&self, article: i64) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT deleted FROM articles WHERE id = ?1",
            [article],
            |row| row.get(0),
        )?)
    }

    /// Move an article to Trash, or restore it, retaining content and notes.
    ///
    /// Returns whether the state changed; repeated requests are idempotent.
    ///
    /// # Errors
    /// Returns missing-article or SQLite errors without a partial mutation.
    pub fn set_deleted(&self, article: i64, deleted: bool) -> Result<bool> {
        let transaction = self.connection.unchecked_transaction()?;
        let previous: bool = transaction.query_row(
            "SELECT deleted FROM articles WHERE id = ?1",
            [article],
            |row| row.get(0),
        )?;
        if previous == deleted {
            return Ok(false);
        }
        restore(&transaction, article, deleted)?;
        history::record(
            &transaction,
            &Undo::Deletion {
                article,
                deleted: previous,
            },
        )?;
        transaction.commit()?;
        Ok(true)
    }
}

pub(super) fn restore_batch(
    transaction: &Transaction<'_>,
    articles: &[i64],
) -> Result<()> {
    let count = transaction.execute(
        include_str!("sql/deletion/restore_batch.sql"),
        [serde_json::to_string(articles)?],
    )?;
    ensure!(count == articles.len(), "A batch article no longer exists");
    Ok(())
}

pub(super) fn restore(
    transaction: &Transaction<'_>,
    article: i64,
    deleted: bool,
) -> Result<()> {
    let changed = transaction.execute(
        include_str!("sql/update_article_deleted.sql"),
        (deleted, article),
    )?;
    ensure!(changed == 1, "Article no longer exists: {article}");
    Ok(())
}
