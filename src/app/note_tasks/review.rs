//! Review mutations run on the same ordered writer as annotation edits.

use anyhow::{Context, Result};

use super::super::ReviewTarget;
use crate::store::Store;

pub(in crate::app) fn review(
    store: &Store,
    target: &ReviewTarget,
    days: u16,
    postpone: bool,
) -> Result<i64> {
    let now = chrono::Utc::now().timestamp();
    match target {
        ReviewTarget::Card(card) => {
            let article = store.note_summary(card.comment)?.article;
            let annotation = store.annotation(article, card.comment)?;
            anyhow::ensure!(
                annotation.cards.iter().any(|saved| saved == card.as_ref()),
                "Card changed; reopen its question before grading"
            );
            if postpone {
                store.postpone_annotation(
                    &annotation,
                    Some(card.id),
                    days,
                    now,
                )?;
            } else {
                store.review_study_card(card, days, now)?;
            }
            Ok::<i64, anyhow::Error>(article)
        }
        ReviewTarget::Note(annotation) => {
            if postpone {
                store.postpone_annotation(annotation, None, days, now)?;
            } else {
                store.review_captured_annotation(annotation, days, now)?;
            }
            Ok(annotation.article)
        }
    }
    .context("Could not update the captured review")
}
