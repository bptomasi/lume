//! Editor commits share one captured, recoverable workflow across adapters.
//!
//! Checkpointing precedes the domain transaction so validation/storage failures
//! retain user text. Success consumes that exact checkpoint inside the domain
//! commit and returns the durable identity, never a later latest-row lookup.

use crate::{
    model::{DraftTarget, NoteDraft},
    store::Store,
};
use anyhow::{Context, Result};

impl Store {
    /// Checkpoint and commit a captured note/card buffer against its baseline.
    ///
    /// # Errors
    /// Rejects stale text/sources, invalid ownership/content/references, recovery
    /// conflicts or storage failures. A saved checkpoint remains recoverable.
    pub fn write_note_draft(&self, draft: &NoteDraft) -> Result<DraftTarget> {
        self.save_note_draft(draft)?;
        persist_target(self, draft)
    }
}

fn persist_target(store: &Store, draft: &NoteDraft) -> Result<DraftTarget> {
    Ok(match &draft.target {
        DraftTarget::Note { article, note } => DraftTarget::Note {
            article: *article,
            note: Some(persist_note(store, draft, *article, *note)?),
        },
        DraftTarget::Passage { article, passage } => DraftTarget::Note {
            article: *article,
            note: Some(store.save_typed_passage(
                *article,
                passage,
                &draft.body,
                draft.initial_kind.unwrap_or_default(),
            )?),
        },
        DraftTarget::Card {
            article,
            note,
            card,
        } => {
            let id = persist_card(store, draft, *article, *note, *card)?;

            DraftTarget::Card {
                article: *article,
                note: *note,
                card: Some(id),
            }
        }
    })
}

fn persist_note(
    store: &Store,
    draft: &NoteDraft,
    article: i64,
    note: Option<i64>,
) -> Result<i64> {
    if note.is_none()
        && let Some(kind) = draft.initial_kind
    {
        return store.save_composed_note(article, &draft.body, kind);
    }
    store.save_comment_checked(
        article,
        note,
        &draft.body,
        Some(&draft.baseline),
    )
}

fn persist_card(
    store: &Store,
    draft: &NoteDraft,
    article: i64,
    note: i64,
    card: Option<i64>,
) -> Result<i64> {
    let (question, answer) = draft
        .body
        .split_once('|')
        .context("Use Question | Answer; both must be your own study text")?;
    if let Some(card) = card {
        return store.update_study_card_checked(
            article,
            note,
            card,
            question,
            answer,
            Some(&draft.baseline),
        );
    }
    store.create_study_card(article, note, question, answer)
}
