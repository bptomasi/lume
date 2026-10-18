//! Atomic captured batches and independent tag ownership, with persistent undo.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use rusqlite::{Connection, Transaction};

use crate::model::{BulkEdit, TagTarget};

use super::{
    Store,
    history::{self, Undo},
};

impl Store {
    /// Return an article's or annotation's independent, sorted tags.
    ///
    /// # Errors
    /// Returns SQLite errors; a missing target has no tags.
    pub fn tags(&self, target: TagTarget) -> Result<Vec<String>> {
        tags(&self.connection, target)
    }

    /// Replace independent tags and record their prior state atomically.
    ///
    /// # Errors
    /// Returns invalid names, missing owners or SQLite failures.
    pub fn set_tags(&self, target: TagTarget, values: &[String]) -> Result<()> {
        let validated = validate_tags(values)?;
        let transaction = self.connection.unchecked_transaction()?;
        let previous = tags(&transaction, target)?;
        replace_tags(&transaction, target, &validated)?;
        history::record(
            &transaction,
            &Undo::Tags {
                target,
                tags: previous,
            },
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Apply one reversible operation to at most 1,000 captured active articles.
    ///
    /// # Errors
    /// Any missing/deleted target or mutation failure rolls back the whole batch.
    pub fn edit_articles(
        &self,
        articles: &BTreeSet<i64>,
        edit: &BulkEdit,
    ) -> Result<()> {
        ensure!(
            !articles.is_empty() && articles.len() <= 1000,
            "Select 1–1,000 articles"
        );
        let transaction = self.connection.unchecked_transaction()?;
        let mut previous = Vec::new();
        for article in articles {
            let deleted: bool = transaction.query_row(
                "SELECT deleted FROM articles WHERE id = ?1",
                [article],
                |row| row.get(0),
            )?;
            ensure!(!deleted, "An article moved to Trash; batch cancelled");
            previous.push(apply_edit(&transaction, *article, edit)?);
        }
        history::record(&transaction, &Undo::Batch(previous))?;
        transaction.commit()?;
        Ok(())
    }
}

fn apply_edit(
    transaction: &Transaction<'_>,
    article: i64,
    edit: &BulkEdit,
) -> Result<Undo> {
    match edit {
        BulkEdit::Unread(value) => flag(transaction, article, "unread", *value),
        BulkEdit::Favorite(value) => {
            flag(transaction, article, "starred", *value)
        }
        BulkEdit::Queue(value) => flag(transaction, article, "queued", *value),
        BulkEdit::Folder(folder) => {
            let lists = transaction
                .prepare_cached(
                    "SELECT collection_id FROM members WHERE article_id = ?1",
                )?
                .query_map([article], |row| row.get(0))?
                .collect::<rusqlite::Result<_>>()?;
            transaction.execute(
                include_str!("sql/insert_list_member.sql"),
                (folder, article),
            )?;
            Ok(Undo::Memberships { article, lists })
        }
        BulkEdit::Tag(tag) => {
            let target = TagTarget::Article(article);
            let previous = tags(transaction, target)?;
            let mut combined = previous.clone();
            combined.push(tag.clone());
            replace_tags(transaction, target, &validate_tags(&combined)?)?;
            Ok(Undo::Tags {
                target,
                tags: previous,
            })
        }
    }
}

fn flag(
    transaction: &Transaction<'_>,
    article: i64,
    column: &str,
    value: bool,
) -> Result<Undo> {
    let previous = transaction.query_row(
        "SELECT unread, starred, queued FROM articles WHERE id = ?1",
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
    transaction.execute(
        &format!("UPDATE articles SET {column} = ?1 WHERE id = ?2"),
        (value, article),
    )?;
    Ok(previous)
}

pub(super) fn tags(
    connection: &Connection,
    target: TagTarget,
) -> Result<Vec<String>> {
    let (table, column, id) = tag_owner(target);
    let sql = format!(
        "SELECT tag FROM {table} WHERE {column} = ?1 ORDER BY tag COLLATE NOCASE"
    );
    Ok(connection
        .prepare_cached(&sql)?
        .query_map([id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

pub(super) fn replace_tags(
    transaction: &Transaction<'_>,
    target: TagTarget,
    values: &[String],
) -> Result<()> {
    let (table, column, id) = tag_owner(target);
    let owner = match target {
        TagTarget::Article(_) => "articles",
        TagTarget::Comment(_) => "comments",
    };
    transaction.query_row(
        &format!("SELECT id FROM {owner} WHERE id = ?1"),
        [id],
        |row| row.get::<_, i64>(0),
    )?;
    transaction
        .execute(&format!("DELETE FROM {table} WHERE {column} = ?1"), [id])?;
    for value in values {
        transaction.execute(
            &format!("INSERT INTO {table} ({column}, tag) VALUES (?1, ?2)"),
            (id, value),
        )?;
    }
    Ok(())
}

fn tag_owner(target: TagTarget) -> (&'static str, &'static str, i64) {
    match target {
        TagTarget::Article(id) => ("article_tags", "article_id", id),
        TagTarget::Comment(id) => ("comment_tags", "comment_id", id),
    }
}

fn validate_tags(values: &[String]) -> Result<Vec<String>> {
    ensure!(values.len() <= 64, "At most 64 tags per owner");
    let mut unique = BTreeSet::new();
    for value in values {
        let value = value.trim().to_ascii_lowercase();
        ensure!(
            !value.is_empty()
                && value.len() <= 64
                && value.chars().all(
                    |c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.')
                ),
            "Tags use 1–64 letters, digits, dots, hyphens or underscores"
        );
        unique.insert(value);
    }
    Ok(unique.into_iter().collect())
}
