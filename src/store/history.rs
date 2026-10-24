//! Transactional undo snapshots for personal edits, never publisher refreshes.
//!
//! A bounded persistent stack survives restart. The inverse and its journal
//! removal commit together; failed recovery leaves the undo record available.

use anyhow::Result;
use rusqlite::{OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::model::{Collection, Comment};

use super::Store;

mod purge;
pub(super) use purge::forget_articles;

#[derive(Serialize, Deserialize)]
pub(super) enum Undo {
    Annotation(Box<crate::model::AnnotationSnapshot>),
    Batch(Vec<Undo>),
    Tags {
        target: crate::model::TagTarget,
        tags: Vec<String>,
    },
    Deletion {
        article: i64,
        deleted: bool,
    },
    DeletionBatch {
        articles: Vec<i64>,
    },
    Flags {
        article: i64,
        unread: bool,
        starred: bool,
        queued: bool,
    },
    Comment {
        article: i64,
        comment: Comment,
    },
    DeleteComment {
        article: i64,
        comment: i64,
    },
    Collection(Collection),
    DeleteCollection(i64),
    Memberships {
        article: i64,
        lists: Vec<i64>,
    },
}

pub(super) fn record(transaction: &Transaction<'_>, undo: &Undo) -> Result<()> {
    transaction.execute(
        "INSERT INTO undo_log (operation, payload) VALUES ('personal', ?1)",
        [serde_json::to_string(undo)?],
    )?;
    transaction.execute(
        r#"
        DELETE FROM undo_log
        WHERE id NOT IN (SELECT id FROM undo_log ORDER BY id DESC LIMIT 256)
        "#,
        [],
    )?;
    Ok(())
}

impl Store {
    /// Restore the latest reversible personal edit, across application restarts.
    ///
    /// # Errors
    /// Returns missing-target, invalid history or SQLite errors without popping
    /// the failed history entry.
    pub fn undo(&self) -> Result<bool> {
        self.undo_effect()
            .map(|effect| effect != crate::model::UndoEffect::Unchanged)
    }

    /// Restore a personal edit and report the smallest invalidated cache scope.
    ///
    /// # Errors
    /// Returns the same atomic recovery/ownership failures as `undo`.
    pub fn undo_effect(&self) -> Result<crate::model::UndoEffect> {
        let transaction = self.connection.unchecked_transaction()?;
        let last: Option<(i64, String)> = transaction
            .query_row(
                "SELECT id, payload FROM undo_log ORDER BY id DESC LIMIT 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((id, payload)) = last else {
            return Ok(crate::model::UndoEffect::Unchanged);
        };
        let undo = serde_json::from_str(&payload)?;
        let effect = match &undo {
            Undo::Annotation(annotation) => {
                crate::model::UndoEffect::Notes(annotation.article)
            }
            Undo::Comment { article, .. }
            | Undo::DeleteComment { article, .. } => {
                crate::model::UndoEffect::Notes(*article)
            }
            Undo::Tags {
                target: crate::model::TagTarget::Comment(note),
                ..
            } => {
                let article = transaction.query_row(
                    "SELECT article_id FROM comments WHERE id = ?1",
                    [note],
                    |row| row.get(0),
                )?;
                crate::model::UndoEffect::Notes(article)
            }
            _ => crate::model::UndoEffect::Library,
        };
        restore(&transaction, undo)?;
        transaction.execute("DELETE FROM undo_log WHERE id = ?1", [id])?;
        transaction.commit()?;
        Ok(effect)
    }

    /// Clear fixture/setup history without changing personal data.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn clear_history(&self) -> Result<()> {
        self.connection.execute("DELETE FROM undo_log", [])?;
        Ok(())
    }

    pub(super) fn set_flag(
        &self,
        article: i64,
        column: &str,
        value: bool,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        let undo = transaction.query_row(
            include_str!("sql/select_personal_flags.sql"),
            [article],
            |row| {
                Ok(Undo::Flags {
                    article,
                    unread: row.get(0)?,
                    starred: row.get(1)?,
                    queued: row.get(2)?,
                })
            },
        )?;
        // Column names are owner-selected constants, never user input.
        let sql = include_str!("sql/update_personal_flag.sql")
            .replace("{column}", column);
        if transaction.execute(&sql, (value, article))? > 0 {
            record(&transaction, &undo)?;
        }
        transaction.commit()?;
        Ok(())
    }
}

#[expect(
    clippy::too_many_lines,
    reason = "Keep every inverse visible in one dispatcher under the same undo transaction"
)]
fn restore(transaction: &Transaction<'_>, undo: Undo) -> Result<()> {
    match undo {
        Undo::Annotation(annotation) => {
            restore_comment(
                transaction,
                annotation.article,
                &annotation.comment,
            )?;
            super::study::restore(transaction, &annotation)?;
        }
        Undo::Batch(operations) => {
            for operation in operations.into_iter().rev() {
                restore(transaction, operation)?;
            }
        }
        Undo::Tags { target, tags } => {
            super::organization::replace_tags(transaction, target, &tags)?;
        }
        Undo::Deletion { article, deleted } => {
            super::deletion::restore(transaction, article, deleted)?;
        }
        Undo::DeletionBatch { articles } => {
            super::deletion::restore_batch(transaction, &articles)?;
        }
        Undo::Flags {
            article,
            unread,
            starred,
            queued,
        } => {
            restore_flags(transaction, article, unread, starred, queued)?;
        }
        Undo::Comment { article, comment } => {
            restore_comment(transaction, article, &comment)?
        }
        Undo::DeleteComment { article, comment } => {
            transaction.execute(
                "DELETE FROM comments WHERE id = ?1 AND article_id = ?2",
                (comment, article),
            )?;
        }
        Undo::Collection(collection) => {
            restore_collection(transaction, &collection)?
        }
        Undo::DeleteCollection(id) => {
            transaction
                .execute("DELETE FROM collections WHERE id = ?1", [id])?;
        }
        Undo::Memberships { article, lists } => {
            super::collections::replace_memberships(
                transaction,
                article,
                &lists,
            )?;
        }
    }
    Ok(())
}

fn restore_flags(
    transaction: &Transaction<'_>,
    article: i64,
    unread: bool,
    starred: bool,
    queued: bool,
) -> Result<()> {
    transaction.execute(
        r#"
        UPDATE articles
        SET unread = ?1, starred = ?2, queued = ?3
        WHERE id = ?4
        "#,
        (unread, starred, queued, article),
    )?;
    Ok(())
}

fn restore_comment(
    transaction: &Transaction<'_>,
    article: i64,
    comment: &Comment,
) -> Result<()> {
    let restored = transaction.execute(
        r#"
        INSERT INTO comments (
            id, article_id, body, created, quote, anchor, source, source_revision
        )
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        ON CONFLICT (id) DO UPDATE SET
            body = excluded.body,
            quote = excluded.quote,
            anchor = excluded.anchor,
            source = excluded.source,
            source_revision = excluded.source_revision
        WHERE comments.article_id = excluded.article_id
        "#,
        rusqlite::params![
            comment.id,
            article,
            comment.body,
            comment.created,
            comment.quote,
            comment.anchor,
            comment.source.as_str(),
            comment.source_revision
        ],
    )?;
    anyhow::ensure!(
        restored == 1,
        "Annotation identity was reused by another article"
    );
    Ok(())
}

fn restore_collection(
    transaction: &Transaction<'_>,
    collection: &Collection,
) -> Result<()> {
    transaction.execute(
        r#"
        INSERT INTO collections (id, name) VALUES (?1, ?2)
        ON CONFLICT (id) DO UPDATE SET name = excluded.name
        "#,
        (collection.id, &collection.name),
    )?;
    transaction.execute(
        "DELETE FROM members WHERE collection_id = ?1",
        [collection.id],
    )?;
    for article in &collection.articles {
        transaction.execute(
            "INSERT INTO members (collection_id, article_id) VALUES (?1, ?2)",
            (collection.id, article),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unchanged_flags_never_write_or_create_false_retention_protections()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = Store::open(&directory.path().join("library.sqlite3"))?;
        store
            .connection
            .execute_batch(include_str!("sql/test_setup_retention.sql"))?;
        let before = store.connection.total_changes();
        store.set_unread(1, false)?;
        store.set_starred(1, false)?;
        store.set_queued(1, false)?;
        assert_eq!(store.connection.total_changes(), before);
        assert!(!store.undo()?);
        store.set_queued(1, true)?;
        assert!(store.article(1)?.queued);
        assert!(store.undo()?);
        assert!(!store.article(1)?.queued);
        Ok(())
    }
}
