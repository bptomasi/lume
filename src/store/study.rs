//! User-authored cards, explicit review dates and reversible annotation context.
//!
//! A review interval is chosen by the user, not predicted by a learning model.
//! All mutation snapshots include tags, links, schedules and cards so undoing
//! an annotation deletion cannot lose the study material it owned.

use anyhow::{Result, ensure};
use rusqlite::{Connection, OptionalExtension, Row, Transaction};

use crate::model::{AnnotationSnapshot, ReviewSchedule, StudyCard, TagTarget};

use super::{
    Store,
    history::{self, Undo},
};

impl Store {
    /// Read complete annotation context, checking article ownership.
    ///
    /// # Errors
    /// Returns a missing/wrong owner, corrupt state or SQLite error.
    pub fn annotation(
        &self,
        article: i64,
        comment: i64,
    ) -> Result<AnnotationSnapshot> {
        snapshot(&self.connection, article, comment)
    }

    /// Link an annotation to an active related article, preserving earlier links.
    ///
    /// # Errors
    /// Returns missing targets, too many links or SQLite failures.
    pub fn link_annotation(
        &self,
        article: i64,
        comment: i64,
        related: i64,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        let previous = active_snapshot(&transaction, article, comment)?;
        ensure!(previous.links.len() < 128, "Annotation link limit is 128");
        let active: bool = transaction.query_row(
            "SELECT deleted = 0 FROM articles WHERE id = ?1",
            [related],
            |row| row.get(0),
        )?;
        ensure!(active, "Restore the related article before linking it");
        transaction.execute(
            r#"
            INSERT OR IGNORE INTO note_links (comment_id, article_id)
            VALUES (?1, ?2)
            "#,
            (comment, related),
        )?;
        history::record(&transaction, &Undo::Annotation(Box::new(previous)))?;
        transaction.commit()?;
        Ok(())
    }

    /// Create a card from the user's question/answer on a captured annotation.
    ///
    /// # Errors
    /// Returns wrong ownership, invalid text, card limits or SQLite failures.
    pub fn create_study_card(
        &self,
        article: i64,
        comment: i64,
        question: &str,
        answer: &str,
    ) -> Result<i64> {
        validate_card(question, answer)?;
        let transaction = self.write_transaction()?;
        let previous = active_snapshot(&transaction, article, comment)?;
        ensure!(
            previous.cards.len() < 16,
            "An annotation can own at most 16 study cards"
        );
        transaction.execute(
            include_str!("sql/insert_study_card.sql"),
            (comment, question.trim(), answer.trim()),
        )?;
        let id = transaction.last_insert_rowid();
        clear_card_draft(
            &transaction,
            article,
            comment,
            None,
            question,
            answer,
        )?;
        history::record(&transaction, &Undo::Annotation(Box::new(previous)))?;
        transaction.commit()?;
        Ok(id)
    }

    /// Update an owned card's text, retaining its identity and review schedule.
    ///
    /// # Errors
    /// Returns invalid text, wrong ownership, missing cards or SQLite failures.
    pub fn update_study_card(
        &self,
        article: i64,
        comment: i64,
        card: i64,
        question: &str,
        answer: &str,
    ) -> Result<i64> {
        self.update_study_card_checked(
            article, comment, card, question, answer, None,
        )
    }

    /// Update an owned editor card only while its committed text is unchanged.
    ///
    /// # Errors
    /// Rejects stale text, invalid content, wrong ownership or SQLite failures.
    pub fn update_study_card_checked(
        &self,
        article: i64,
        comment: i64,
        card: i64,
        question: &str,
        answer: &str,
        baseline: Option<&str>,
    ) -> Result<i64> {
        validate_card(question, answer)?;
        let transaction = self.write_transaction()?;
        let previous = active_snapshot(&transaction, article, comment)?;
        ensure!(
            previous.cards.iter().any(|item| item.id == card),
            "Card no longer belongs to this annotation"
        );
        if let Some(text) = baseline {
            let saved = previous
                .cards
                .iter()
                .find(|item| item.id == card)
                .ok_or_else(|| {
                    anyhow::anyhow!("Card no longer belongs to this annotation")
                })?;
            ensure!(
                text.split_once('|')
                    .is_some_and(|(question, answer)| question.trim()
                        == saved.question
                        && answer.trim() == saved.answer),
                "Saved card changed; compare the draft before replacing it"
            );
        }
        transaction.execute(
            include_str!("sql/update_study_card_text.sql"),
            (question.trim(), answer.trim(), card, comment),
        )?;
        clear_card_draft(
            &transaction,
            article,
            comment,
            Some(card),
            question,
            answer,
        )?;
        history::record(&transaction, &Undo::Annotation(Box::new(previous)))?;
        transaction.commit()?;
        Ok(card)
    }

    /// Schedule the selected passage after an explicit successful review.
    ///
    /// # Errors
    /// Returns invalid interval/time, ownership or SQLite errors.
    pub fn review_annotation(
        &self,
        article: i64,
        comment: i64,
        days: u16,
        now: i64,
    ) -> Result<()> {
        let due = review_deadline(days, now)?;
        let transaction = self.write_transaction()?;
        let previous = active_snapshot(&transaction, article, comment)?;
        write_annotation_review(&transaction, previous, days, due)?;
        transaction.commit()?;
        Ok(())
    }

    /// Review the exact passage/note and schedule shown in a captured preview.
    ///
    /// # Errors
    /// Rejects changed/deleted notes, Trash, concurrent reviews or invalid dates.
    /// No review or history entry is written when the preview is stale.
    pub fn review_captured_annotation(
        &self,
        annotation: &AnnotationSnapshot,
        days: u16,
        now: i64,
    ) -> Result<()> {
        let due = review_deadline(days, now)?;
        let transaction = self.write_transaction()?;
        let previous = active_snapshot(
            &transaction,
            annotation.article,
            annotation.comment.id,
        )?;
        ensure!(
            previous.comment == annotation.comment
                && previous.review == annotation.review,
            "Passage or review changed; reopen the review preview"
        );
        write_annotation_review(&transaction, previous, days, due)?;
        transaction.commit()?;
        Ok(())
    }

    /// Reschedule one user-authored card without altering its answer.
    ///
    /// # Errors
    /// Returns a missing card, invalid interval or SQLite error.
    pub fn review_study_card(
        &self,
        card: &StudyCard,
        days: u16,
        now: i64,
    ) -> Result<()> {
        let due = review_deadline(days, now)?;
        let transaction = self.write_transaction()?;
        let (article, comment) = transaction.query_row(
            include_str!("sql/select_study_card_owner.sql"),
            [card.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        let previous = active_snapshot(&transaction, article, comment)?;
        ensure!(
            previous.cards.iter().any(|current| current == card),
            "Study card changed; reopen its question before reviewing"
        );
        transaction.execute(
            include_str!("sql/update_study_card_review.sql"),
            (due, days, card.id),
        )?;
        history::record(&transaction, &Undo::Annotation(Box::new(previous)))?;
        transaction.commit()?;
        Ok(())
    }

    /// Return up to 200 due passage identities, excluding Trash.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn due_annotations(&self, now: i64) -> Result<Vec<(i64, i64)>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/select_due_annotations.sql"))?
            .query_map([now], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Return up to 200 due cards, excluding cards attached to Trash articles.
    ///
    /// # Errors
    /// Returns SQLite or stored integer decoding errors.
    pub fn due_study_cards(&self, now: i64) -> Result<Vec<StudyCard>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/select_due_study_cards.sql"))?
            .query_map([now], card_row)?
            .collect::<rusqlite::Result<_>>()?)
    }
}

fn clear_card_draft(
    transaction: &Transaction<'_>,
    article: i64,
    note: i64,
    card: Option<i64>,
    question: &str,
    answer: &str,
) -> Result<()> {
    super::notebook::clear_written_draft(
        transaction,
        &crate::model::DraftTarget::Card {
            article,
            note,
            card,
        },
        &format!("{question}|{answer}"),
    )
}

pub(super) fn active_snapshot(
    connection: &Connection,
    article: i64,
    comment: i64,
) -> Result<AnnotationSnapshot> {
    let active: bool = connection.query_row(
        "SELECT deleted = 0 FROM articles WHERE id = ?1",
        [article],
        |row| row.get(0),
    )?;
    ensure!(active, "Restore the article before changing study material");
    snapshot(connection, article, comment)
}

fn write_annotation_review(
    transaction: &Transaction<'_>,
    previous: AnnotationSnapshot,
    days: u16,
    due: i64,
) -> Result<()> {
    transaction.execute(
        include_str!("sql/insert_annotation_review.sql"),
        (previous.comment.id, due, days),
    )?;
    history::record(transaction, &Undo::Annotation(Box::new(previous)))
}

pub(super) fn snapshot(
    connection: &Connection,
    article: i64,
    comment: i64,
) -> Result<AnnotationSnapshot> {
    let comment = connection.query_row(
        include_str!("sql/select_owned_comment.sql"),
        (article, comment),
        super::comments::comment_row,
    )?;
    let tags =
        super::organization::tags(connection, TagTarget::Comment(comment.id))?;
    let links = connection
        .prepare_cached(
            r#"
        SELECT article_id
        FROM note_links
        WHERE comment_id = ?1
        ORDER BY article_id
        "#,
        )?
        .query_map([comment.id], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?;
    let review = review_schedule(connection, comment.id)?;
    let cards = connection
        .prepare_cached(include_str!("sql/select_annotation_cards.sql"))?
        .query_map([comment.id], card_row)?
        .collect::<rusqlite::Result<_>>()?;
    let metadata = super::notebook::read_metadata(connection, comment.id)?;
    Ok(AnnotationSnapshot {
        article,
        comment,
        tags,
        links,
        review,
        cards,
        metadata,
    })
}

pub(super) fn restore(
    transaction: &Transaction<'_>,
    annotation: &AnnotationSnapshot,
) -> Result<()> {
    let comment = annotation.comment.id;
    super::organization::replace_tags(
        transaction,
        TagTarget::Comment(comment),
        &annotation.tags,
    )?;
    transaction
        .execute("DELETE FROM note_links WHERE comment_id = ?1", [comment])?;
    for article in &annotation.links {
        transaction.execute(
            "INSERT INTO note_links (comment_id, article_id) VALUES (?1, ?2)",
            (comment, article),
        )?;
    }
    transaction.execute(
        "DELETE FROM annotation_reviews WHERE comment_id = ?1",
        [comment],
    )?;
    if let Some(review) = &annotation.review {
        transaction.execute(
            include_str!("sql/insert_restore_annotation_review.sql"),
            (comment, review.due_at, review.interval_days, review.reviews),
        )?;
    }
    restore_cards(transaction, comment, &annotation.cards)?;
    super::notebook::restore_metadata(
        transaction,
        comment,
        &annotation.metadata,
    )?;
    Ok(())
}

pub(super) fn review_deadline(days: u16, now: i64) -> Result<i64> {
    ensure!(
        (1..=3650).contains(&days),
        "Choose a review interval of 1–3,650 days"
    );
    now.checked_add(i64::from(days) * 86_400)
        .ok_or_else(|| anyhow::anyhow!("Review date overflow"))
}

fn validate_card(question: &str, answer: &str) -> Result<()> {
    ensure!(
        !question.trim().is_empty() && question.len() <= 1024,
        "Question must contain 1–1,024 bytes"
    );
    ensure!(
        !answer.trim().is_empty() && answer.len() <= 8192,
        "Answer must contain 1–8,192 bytes"
    );
    ensure!(
        crate::content::clean(question) == question
            && crate::content::clean(answer) == answer,
        "Study text contains terminal controls"
    );
    Ok(())
}

pub(super) fn card_row(row: &Row<'_>) -> rusqlite::Result<StudyCard> {
    Ok(StudyCard {
        id: row.get(0)?,
        comment: row.get(1)?,
        question: row.get(2)?,
        answer: row.get(3)?,
        due_at: row.get(4)?,
        interval_days: row.get(5)?,
        reviews: row.get(6)?,
    })
}

fn restore_cards(
    transaction: &Transaction<'_>,
    comment: i64,
    cards: &[StudyCard],
) -> Result<()> {
    transaction
        .execute("DELETE FROM study_cards WHERE comment_id = ?1", [comment])?;
    for card in cards {
        transaction.execute(
            include_str!("sql/insert_restore_study_card.sql"),
            (
                card.id,
                comment,
                &card.question,
                &card.answer,
                card.due_at,
                card.interval_days,
                card.reviews,
            ),
        )?;
    }
    Ok(())
}

fn review_schedule(
    connection: &Connection,
    comment: i64,
) -> Result<Option<ReviewSchedule>> {
    Ok(connection
        .query_row(
            r#"
        SELECT due_at, interval_days, reviews
        FROM annotation_reviews
        WHERE comment_id = ?1
        "#,
            [comment],
            |row| {
                Ok(ReviewSchedule {
                    due_at: row.get(0)?,
                    interval_days: row.get(1)?,
                    reviews: row.get(2)?,
                })
            },
        )
        .optional()?)
}
