//! Private notes, immutable quoted passages and reversible editing.

use anyhow::{Context, Result, ensure};
use rusqlite::{OptionalExtension, Row, Transaction};

use crate::model::{Comment, ContentSource, Passage};

use super::{
    Store,
    history::{self, Undo},
};

impl Store {
    /// Read comments in creation order, with captured quotations.
    ///
    /// # Errors
    /// Returns SQLite or decoding errors.
    pub fn comments(&self, article: i64) -> Result<Vec<Comment>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/select_article_comments.sql"))?
            .query_map([article], comment_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Save a note or update its body without changing its captured quote.
    ///
    /// # Errors
    /// Returns invalid content, wrong article or SQLite errors.
    pub fn save_comment(
        &self,
        article: i64,
        comment: Option<i64>,
        body: &str,
    ) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        if let Some(id) = comment {
            let previous = snapshot(&transaction, article, id)?;
            validate(body, &previous.quote)?;
            history::record(
                &transaction,
                &Undo::Comment {
                    article,
                    comment: previous,
                },
            )?;
            transaction.execute(
                include_str!("sql/update_comment_body.sql"),
                (body.trim(), id, article),
            )?;
        } else {
            validate(body, "")?;
            insert(
                &transaction,
                article,
                body,
                &Passage {
                    quote: String::new(),
                    anchor: 0,
                    source: ContentSource::Feed,
                    revision: 0,
                },
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Save a highlighted passage and an optional note as one annotation.
    ///
    /// # Errors
    /// Returns empty/oversized content, invalid offsets or SQLite errors.
    pub fn save_highlight(
        &self,
        article: i64,
        quote: &str,
        anchor: usize,
        note: &str,
    ) -> Result<()> {
        self.save_passage(
            article,
            &Passage {
                quote: quote.to_owned(),
                anchor,
                source: ContentSource::Feed,
                revision: 0,
            },
            note,
        )
    }

    /// Save a captured passage without confusing Feed and Full revisions.
    ///
    /// # Errors
    /// Returns invalid content, a changed Full source or SQLite errors.
    pub fn save_passage(
        &self,
        article: i64,
        passage: &Passage,
        note: &str,
    ) -> Result<()> {
        validate(note, &passage.quote)?;
        ensure!(!passage.quote.trim().is_empty(), "Select a passage first");
        let transaction = self.connection.unchecked_transaction()?;
        validate_source(&transaction, article, passage)?;
        insert(&transaction, article, note, passage)?;
        transaction.commit()?;
        Ok(())
    }

    /// Delete a comment while recording enough context to restore it.
    ///
    /// # Errors
    /// Returns missing/wrong article identities or SQLite errors.
    pub fn delete_comment(&self, article: i64, comment: i64) -> Result<()> {
        let transaction = self.connection.unchecked_transaction()?;
        let previous = super::study::snapshot(&transaction, article, comment)?;
        history::record(&transaction, &Undo::Annotation(Box::new(previous)))?;
        transaction.execute(
            "DELETE FROM comments WHERE article_id = ?1 AND id = ?2",
            (article, comment),
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn validate(note: &str, quote: &str) -> Result<()> {
    ensure!(
        !note.trim().is_empty() || !quote.trim().is_empty(),
        "Comment cannot be empty"
    );
    ensure!(note.len() <= 16_384, "Comment exceeds 16 KiB");
    ensure!(
        note.chars()
            .all(|c| !c.is_control() || matches!(c, '\n' | '\t')),
        "Comment contains terminal control characters"
    );
    ensure!(quote.len() <= 8192, "Highlighted passage exceeds 8 KiB");
    Ok(())
}

fn insert(
    transaction: &Transaction<'_>,
    article: i64,
    note: &str,
    passage: &Passage,
) -> Result<()> {
    transaction.execute(
        include_str!("sql/insert_article_comment.sql"),
        (
            article,
            note.trim(),
            &passage.quote,
            i64::try_from(passage.anchor)?,
            passage.source.as_str(),
            passage.revision,
        ),
    )?;
    history::record(
        transaction,
        &Undo::DeleteComment {
            article,
            comment: transaction.last_insert_rowid(),
        },
    )
}

fn snapshot(
    transaction: &Transaction<'_>,
    article: i64,
    id: i64,
) -> Result<Comment> {
    Ok(transaction.query_row(
        include_str!("sql/select_owned_comment.sql"),
        (article, id),
        comment_row,
    )?)
}

pub(super) fn comment_row(row: &Row<'_>) -> rusqlite::Result<Comment> {
    Ok(Comment {
        id: row.get(0)?,
        body: row.get(1)?,
        created: row.get(2)?,
        quote: row.get(3)?,
        anchor: row.get(4)?,
        source: if row.get::<_, String>(5)? == "full" {
            ContentSource::Full
        } else {
            ContentSource::Feed
        },
        source_revision: row.get(6)?,
    })
}

fn validate_source(
    transaction: &Transaction<'_>,
    article: i64,
    passage: &Passage,
) -> Result<()> {
    if passage.source == ContentSource::Feed {
        ensure!(passage.revision == 0, "Invalid Feed revision");
        return Ok(());
    }
    let body: String = transaction
        .query_row(
            r#"
        SELECT d.body
        FROM article_documents AS d
        JOIN articles AS a ON a.id = d.article_id
        WHERE a.id = ?1 AND d.url = a.url AND d.revision = ?2
        "#,
            (article, passage.revision),
            |row| row.get(0),
        )
        .optional()?
        .context("Article changed; select the passage again")?;
    ensure!(
        body.get(passage.anchor..)
            .is_some_and(|tail| tail.starts_with(&passage.quote)),
        "Article changed; select the passage again"
    );
    Ok(())
}
