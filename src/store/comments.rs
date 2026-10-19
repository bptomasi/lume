//! Private notes, immutable quoted passages and reversible editing.
//!
//! User Markdown retains indentation and trailing newlines. Its persisted
//! bytes, reference parsing and editor baseline must describe the same text.

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

    /// Save a note and return its durable ID, preserving an existing quote.
    ///
    /// # Errors
    /// Returns invalid content, wrong article or SQLite errors.
    pub fn save_comment(
        &self,
        article: i64,
        comment: Option<i64>,
        body: &str,
    ) -> Result<i64> {
        self.save_comment_checked(article, comment, body, None)
    }

    /// Save an editor buffer only if its last committed text is still current.
    ///
    /// # Errors
    /// Rejects stale buffers without changing the saved note or its history.
    pub fn save_comment_checked(
        &self,
        article: i64,
        comment: Option<i64>,
        body: &str,
        baseline: Option<&str>,
    ) -> Result<i64> {
        let transaction = self.connection.unchecked_transaction()?;
        let id = if let Some(id) = comment {
            update(&transaction, article, id, body, baseline)?;
            id
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
            )?
        };
        transaction.commit()?;
        Ok(id)
    }

    /// Create an explicitly typed general note and its references atomically.
    ///
    /// # Errors
    /// Rejects invalid content/references, missing articles or SQLite failures.
    pub fn save_composed_note(
        &self,
        article: i64,
        body: &str,
        kind: crate::model::NoteKind,
    ) -> Result<i64> {
        validate(body, "")?;
        let transaction = self.connection.unchecked_transaction()?;
        let id = insert(
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
        transaction.execute(
            "UPDATE note_metadata SET kind = ?1 WHERE comment_id = ?2",
            (kind.name(), id),
        )?;
        transaction.commit()?;
        Ok(id)
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
        .map(|_| ())
    }

    /// Save a captured passage and return its ID, retaining source provenance.
    ///
    /// # Errors
    /// Returns invalid content, a changed Full source or SQLite errors.
    pub fn save_passage(
        &self,
        article: i64,
        passage: &Passage,
        note: &str,
    ) -> Result<i64> {
        self.save_typed_passage(
            article,
            passage,
            note,
            crate::model::NoteKind::Insight,
        )
    }

    /// Create a typed passage note with its source and references atomically.
    ///
    /// # Errors
    /// Rejects invalid content/provenance, Trash or SQLite failures.
    pub fn save_typed_passage(
        &self,
        article: i64,
        passage: &Passage,
        note: &str,
        kind: crate::model::NoteKind,
    ) -> Result<i64> {
        validate(note, &passage.quote)?;
        ensure!(!passage.quote.trim().is_empty(), "Select a passage first");
        let transaction = self.connection.unchecked_transaction()?;
        validate_source(&transaction, article, passage)?;
        let id = insert(&transaction, article, note, passage)?;
        if kind != crate::model::NoteKind::Insight {
            transaction.execute(
                "UPDATE note_metadata SET kind = ?1 WHERE comment_id = ?2",
                (kind.name(), id),
            )?;
        }
        transaction.commit()?;
        Ok(id)
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

pub(super) fn validate(note: &str, quote: &str) -> Result<()> {
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

fn update(
    transaction: &Transaction<'_>,
    article: i64,
    id: i64,
    body: &str,
    baseline: Option<&str>,
) -> Result<()> {
    let previous = super::study::active_snapshot(transaction, article, id)?;
    ensure!(
        baseline.is_none_or(|text| text == previous.comment.body),
        "Saved note changed; recover the draft and compare versions before replacing it"
    );
    validate(body, &previous.comment.quote)?;
    history::record(transaction, &Undo::Annotation(Box::new(previous)))?;
    transaction.execute(
        include_str!("sql/update_comment_body.sql"),
        (body, id, article),
    )?;
    super::notebook::replace_references(transaction, id, body)?;
    super::notebook::clear_written_draft(
        transaction,
        &crate::model::DraftTarget::Note {
            article,
            note: Some(id),
        },
        body,
    )?;
    Ok(())
}

fn require_active_article(
    transaction: &Transaction<'_>,
    article: i64,
) -> Result<()> {
    let active: bool = transaction
        .query_row(
            "SELECT deleted = 0 FROM articles WHERE id = ?1",
            [article],
            |row| row.get(0),
        )
        .context("Select an existing article before annotating")?;
    ensure!(active, "Restore the article from Trash before annotating");
    Ok(())
}

fn insert(
    transaction: &Transaction<'_>,
    article: i64,
    note: &str,
    passage: &Passage,
) -> Result<i64> {
    require_active_article(transaction, article)?;
    transaction.execute("INSERT INTO note_identities DEFAULT VALUES", [])?;
    let id = transaction.last_insert_rowid();
    transaction.execute(
        include_str!("sql/insert_article_comment.sql"),
        (
            id,
            article,
            note,
            &passage.quote,
            i64::try_from(passage.anchor)?,
            passage.source.as_str(),
            passage.revision,
        ),
    )?;
    super::notebook::replace_references(transaction, id, note)?;
    let target = if passage.quote.is_empty() {
        crate::model::DraftTarget::Note {
            article,
            note: None,
        }
    } else {
        crate::model::DraftTarget::Passage {
            article,
            passage: passage.clone(),
        }
    };
    super::notebook::clear_written_draft(transaction, &target, note)?;
    history::record(
        transaction,
        &Undo::DeleteComment {
            article,
            comment: id,
        },
    )?;
    Ok(id)
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

pub(super) fn validate_source(
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
