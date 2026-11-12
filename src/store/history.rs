//! Transactional undo snapshots for personal edits, never publisher refreshes.
//!
//! A bounded persistent stack survives restart. The inverse and its journal
//! removal commit together; failed recovery leaves the undo record available.

use anyhow::Result;
use rusqlite::{OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};

use crate::model::{Collection, Comment};

use super::{PersonalFlag, Store};

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
        include_str!("sql/history/insert_undo.sql"),
        [serde_json::to_string(undo)?],
    )?;
    transaction.execute(include_str!("sql/history/delete_old_undo.sql"), [])?;
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
        let transaction = self.write_transaction()?;
        let last: Option<(i64, String)> = transaction
            .query_row(
                include_str!("sql/history/select_latest_undo.sql"),
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
                    include_str!("sql/history/select_note_article.sql"),
                    [note],
                    |row| row.get(0),
                )?;
                crate::model::UndoEffect::Notes(article)
            }
            _ => crate::model::UndoEffect::Library,
        };
        restore(&transaction, undo)?;
        transaction
            .execute(include_str!("sql/history/delete_undo.sql"), [id])?;
        transaction.commit()?;
        Ok(effect)
    }

    /// Clear fixture/setup history without changing personal data.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn clear_history(&self) -> Result<()> {
        self.connection
            .execute(include_str!("sql/history/delete_all_undo.sql"), [])?;
        Ok(())
    }

    pub(super) fn set_flag(
        &self,
        article: i64,
        flag: PersonalFlag,
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
        if transaction.execute(flag.update_one(), (value, article))? > 0 {
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
                include_str!("sql/history/delete_comment.sql"),
                (comment, article),
            )?;
        }
        Undo::Collection(collection) => {
            restore_collection(transaction, &collection)?
        }
        Undo::DeleteCollection(id) => {
            transaction.execute(
                include_str!("sql/history/delete_collection.sql"),
                [id],
            )?;
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
        include_str!("sql/history/restore_flags.sql"),
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
        include_str!("sql/history/restore_comment.sql"),
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
        include_str!("sql/history/restore_collection.sql"),
        (collection.id, &collection.name),
    )?;
    transaction.execute(
        include_str!("sql/history/delete_collection_members.sql"),
        [collection.id],
    )?;
    for article in &collection.articles {
        transaction.execute(
            include_str!("sql/history/insert_member.sql"),
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

    fn flag_fixture() -> Result<(tempfile::TempDir, Store)> {
        let directory = tempfile::tempdir()?;
        let store = Store::open(&directory.path().join("library.sqlite3"))?;
        store
            .connection
            .execute_batch(include_str!("sql/test_setup_retention.sql"))?;
        Ok((directory, store))
    }

    fn flags(store: &Store, article: i64) -> Result<(bool, bool, bool)> {
        let article = store.article(article)?;
        Ok((article.unread, article.starred, article.queued))
    }

    #[test]
    fn individual_flags_change_only_the_selected_state_and_undo_once()
    -> Result<()> {
        type Setter = fn(&Store, i64, bool) -> Result<()>;
        let setters: [(Setter, (bool, bool, bool)); 3] = [
            (Store::set_unread, (true, false, false)),
            (Store::set_starred, (false, true, false)),
            (Store::set_queued, (false, false, true)),
        ];
        let (_directory, store) = flag_fixture()?;
        store.save_progress(1, 17)?;
        for (set, expected) in setters {
            set(&store, 1, true)?;
            assert_eq!(flags(&store, 1)?, expected);
            let changes = store.connection.total_changes();
            set(&store, 1, true)?;
            assert_eq!(store.connection.total_changes(), changes);
            assert!(store.undo()?);
            assert_eq!(flags(&store, 1)?, (false, false, false));
            assert_eq!(store.progress(1)?, 17);
            assert!(!store.undo()?);
            assert!(set(&store, 999, true).is_err());
            assert!(!store.undo()?);
        }
        Ok(())
    }

    #[test]
    fn captured_flag_batches_preserve_noops_and_undo_as_one_edit() -> Result<()>
    {
        use crate::model::BulkEdit;
        let edits = [
            (BulkEdit::Unread(true), (true, false, false)),
            (BulkEdit::Favorite(true), (false, true, false)),
            (BulkEdit::Queue(true), (false, false, true)),
        ];
        let (_directory, store) = flag_fixture()?;
        for (edit, expected) in edits {
            store.edit_articles(&[1, 2].into(), &edit)?;
            assert_eq!(flags(&store, 1)?, expected);
            assert_eq!(flags(&store, 2)?, expected);
            assert_eq!(flags(&store, 3)?, (false, false, false));
            let changes = store.connection.total_changes();
            store.edit_articles(&[1, 2].into(), &edit)?;
            assert_eq!(store.connection.total_changes(), changes);
            assert!(store.undo()?);
            assert_eq!(flags(&store, 1)?, (false, false, false));
            assert_eq!(flags(&store, 2)?, (false, false, false));
            assert!(!store.undo()?);
            assert!(store.edit_articles(&[1, 999].into(), &edit).is_err());
            assert_eq!(flags(&store, 1)?, (false, false, false));
            assert!(!store.undo()?);
        }
        Ok(())
    }

    #[test]
    fn failed_individual_and_batch_flag_writes_leave_no_history() -> Result<()>
    {
        let (_directory, store) = flag_fixture()?;
        store
            .connection
            .execute_batch(include_str!("sql/test_fail_personal_flags.sql"))?;
        for flag in [
            PersonalFlag::Unread,
            PersonalFlag::Starred,
            PersonalFlag::Queued,
        ] {
            let error = store
                .set_flag(2, flag, true)
                .err()
                .ok_or_else(|| anyhow::anyhow!("Flag failure was ignored"))?;
            assert!(error.to_string().contains("Personal flag write failed"));
            assert_eq!(flags(&store, 2)?, (false, false, false));
            assert!(!store.undo()?);
        }
        for edit in [
            crate::model::BulkEdit::Unread(true),
            crate::model::BulkEdit::Favorite(true),
            crate::model::BulkEdit::Queue(true),
        ] {
            assert!(store.edit_articles(&[1, 2].into(), &edit).is_err());
            assert_eq!(flags(&store, 1)?, (false, false, false));
            assert_eq!(flags(&store, 2)?, (false, false, false));
            assert!(!store.undo()?);
        }
        Ok(())
    }

    #[test]
    fn failed_flag_history_rolls_back_individual_and_batch_writes() -> Result<()>
    {
        let (_directory, store) = flag_fixture()?;
        store
            .connection
            .execute_batch(include_str!("sql/test_fail_flag_history.sql"))?;
        for flag in [
            PersonalFlag::Unread,
            PersonalFlag::Starred,
            PersonalFlag::Queued,
        ] {
            let error =
                store.set_flag(1, flag, true).err().ok_or_else(|| {
                    anyhow::anyhow!("History failure was ignored")
                })?;
            assert!(error.to_string().contains("Flag history write failed"));
            assert_eq!(flags(&store, 1)?, (false, false, false));
            assert!(!store.undo()?);
        }
        for edit in [
            crate::model::BulkEdit::Unread(true),
            crate::model::BulkEdit::Favorite(true),
            crate::model::BulkEdit::Queue(true),
        ] {
            assert!(store.edit_articles(&[1, 2].into(), &edit).is_err());
            assert_eq!(flags(&store, 1)?, (false, false, false));
            assert_eq!(flags(&store, 2)?, (false, false, false));
            assert!(!store.undo()?);
        }
        Ok(())
    }

    #[test]
    fn flag_batches_reject_deleted_targets_even_for_noop_values() -> Result<()>
    {
        let (_directory, store) = flag_fixture()?;
        store.set_deleted(2, true)?;
        store.clear_history()?;
        for value in [false, true] {
            for edit in [
                crate::model::BulkEdit::Unread(value),
                crate::model::BulkEdit::Favorite(value),
                crate::model::BulkEdit::Queue(value),
            ] {
                let error = store
                    .edit_articles(&[1, 2].into(), &edit)
                    .err()
                    .ok_or_else(|| {
                        anyhow::anyhow!("Batch admitted a deleted article")
                    })?;
                assert!(error.to_string().contains("moved to Trash"));
                assert_eq!(flags(&store, 1)?, (false, false, false));
                assert_eq!(flags(&store, 2)?, (false, false, false));
                assert!(store.is_deleted(2)?);
                assert!(!store.undo()?);
            }
        }
        Ok(())
    }
}
