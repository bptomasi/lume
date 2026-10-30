//! Atomic captured batches and independent tag ownership, with persistent undo.
//! Unchanged edits preserve the journal; tag updates touch only their difference.

mod flags;

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
        let transaction = self.write_transaction()?;
        let previous = tags(&transaction, target)?;
        if !apply_tags(&transaction, target, &previous, &validated)? {
            return Ok(());
        }
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
        let transaction = self.write_transaction()?;
        let previous = match edit {
            BulkEdit::Unread(value) => {
                flags::apply(&transaction, articles, "unread", *value)?
            }
            BulkEdit::Favorite(value) => {
                flags::apply(&transaction, articles, "starred", *value)?
            }
            BulkEdit::Queue(value) => {
                flags::apply(&transaction, articles, "queued", *value)?
            }
            _ => edit_members(&transaction, articles, edit)?,
        };
        if !previous.is_empty() {
            history::record(&transaction, &Undo::Batch(previous))?;
        }
        transaction.commit()?;
        Ok(())
    }
}

fn edit_members(
    transaction: &Transaction<'_>,
    articles: &BTreeSet<i64>,
    edit: &BulkEdit,
) -> Result<Vec<Undo>> {
    let mut previous = Vec::new();
    for article in articles {
        let deleted: bool = transaction.query_row(
            include_str!("sql/select_article_deleted.sql"),
            [article],
            |row| row.get(0),
        )?;
        ensure!(!deleted, "An article moved to Trash; batch cancelled");
        let undo = match edit {
            BulkEdit::Folder(folder) => {
                add_folder(transaction, *article, *folder)?
            }
            BulkEdit::Tag(tag) => add_tag(transaction, *article, tag)?,
            _ => None,
        };
        previous.extend(undo);
    }
    Ok(previous)
}

fn add_folder(
    transaction: &Transaction<'_>,
    article: i64,
    folder: i64,
) -> Result<Option<Undo>> {
    let lists = transaction
        .prepare_cached(include_str!("sql/select_member_collections.sql"))?
        .query_map([article], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let changed = transaction.execute(
        include_str!("sql/insert_list_member.sql"),
        (folder, article),
    )?;
    Ok((changed > 0).then_some(Undo::Memberships { article, lists }))
}

fn add_tag(
    transaction: &Transaction<'_>,
    article: i64,
    tag: &str,
) -> Result<Option<Undo>> {
    let target = TagTarget::Article(article);
    let previous = tags(transaction, target)?;
    let tag = validate_tags(&[tag.to_owned()])?;
    if tag.iter().all(|value| previous.contains(value)) {
        return Ok(None);
    }
    let mut combined = previous.clone();
    combined.extend(tag);
    let changed =
        apply_tags(transaction, target, &previous, &validate_tags(&combined)?)?;
    Ok(changed.then_some(Undo::Tags {
        target,
        tags: previous,
    }))
}

pub(super) fn tags(
    connection: &Connection,
    target: TagTarget,
) -> Result<Vec<String>> {
    let (table, column, id) = tag_owner(target);
    let sql = include_str!("sql/select_tags.sql")
        .replace("{table}", table)
        .replace("{column}", column);
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
    let previous = tags(transaction, target)?;
    apply_tags(transaction, target, &previous, values)?;
    Ok(())
}

fn apply_tags(
    transaction: &Transaction<'_>,
    target: TagTarget,
    previous: &[String],
    values: &[String],
) -> Result<bool> {
    let (table, column, id) = tag_owner(target);
    let owner = match target {
        TagTarget::Article(_) => "articles",
        TagTarget::Comment(_) => "comments",
    };
    transaction
        .prepare_cached(
            &include_str!("sql/select_tag_owner.sql").replace("{table}", owner),
        )?
        .query_row([id], |row| row.get::<_, i64>(0))?;
    if previous == values {
        return Ok(false);
    }
    let delete = include_str!("sql/delete_tag.sql")
        .replace("{table}", table)
        .replace("{column}", column);
    let insert = include_str!("sql/insert_tag.sql")
        .replace("{table}", table)
        .replace("{column}", column);
    for value in previous.iter().filter(|value| !values.contains(value)) {
        transaction.prepare_cached(&delete)?.execute((id, value))?;
    }
    for value in values.iter().filter(|value| !previous.contains(value)) {
        transaction.prepare_cached(&insert)?.execute((id, value))?;
    }
    Ok(true)
}

fn tag_owner(target: TagTarget) -> (&'static str, &'static str, i64) {
    match target {
        TagTarget::Article(id) => ("article_tags", "article_id", id),
        TagTarget::Comment(id) => ("comment_tags", "comment_id", id),
    }
}

pub(crate) fn validate_tags(values: &[String]) -> Result<Vec<String>> {
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
