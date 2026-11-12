//! Filtering and postponement retain captured review ownership and counts.

use anyhow::{Result, ensure};
use rusqlite::params_from_iter;

use crate::model::{AnnotationSnapshot, StudyCard};
use crate::store::{
    Store,
    history::{self, Undo},
    study,
};

impl Store {
    /// Return at most 200 due cards whose owning note matches the full query.
    ///
    /// # Errors
    /// Returns invalid note queries, stored card data or SQLite errors.
    pub fn filtered_study_cards(
        &self,
        query: &str,
        now: i64,
    ) -> Result<Vec<StudyCard>> {
        let expression = crate::search::notes::compile(query)?;
        let mut values = expression.values;
        values.push(now.into());
        let sql = format!(
            include_str!("../sql/notebook/template_filtered_study_cards.sql"),
            include_str!("../sql/notebook/select_review_cards.sql"),
            expression.sql,
            values.len()
        );
        Ok(self
            .connection
            .prepare_cached(&sql)?
            .query_map(params_from_iter(&values), study::card_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Postpone a captured note/card without incrementing completed reviews.
    ///
    /// # Errors
    /// Rejects stale material/schedules, Trash, invalid dates or SQLite failures.
    pub fn postpone_annotation(
        &self,
        annotation: &AnnotationSnapshot,
        card: Option<i64>,
        days: u16,
        now: i64,
    ) -> Result<()> {
        let due = study::review_deadline(days, now)?;
        let transaction = self.write_transaction()?;
        let previous = study::active_snapshot(
            &transaction,
            annotation.article,
            annotation.comment.id,
        )?;
        ensure!(
            previous.comment == annotation.comment
                && previous.review == annotation.review
                && previous.cards == annotation.cards,
            "Study material changed; reopen the review preview"
        );
        if let Some(id) = card {
            ensure!(
                previous.cards.iter().any(|item| item.id == id),
                "Card no longer belongs to this note"
            );
            transaction.execute(
                include_str!("../sql/notebook/update_card_due.sql"),
                (due, days, id),
            )?;
        } else {
            transaction.execute(
                include_str!("../sql/notebook/insert_postponed_review.sql"),
                (annotation.comment.id, due, days),
            )?;
        }
        history::record(&transaction, &Undo::Annotation(Box::new(previous)))?;
        transaction.commit()?;
        Ok(())
    }
}
