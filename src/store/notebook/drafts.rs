//! Recovery checkpoints never mark a note saved or overwrite another target.

use anyhow::{Result, ensure};
use rusqlite::{OptionalExtension, Transaction};

use crate::{
    model::{DraftTarget, NoteDraft},
    store::Store,
};

impl Store {
    /// Checkpoint a draft independently of committed annotations.
    ///
    /// # Errors
    /// Rejects invalid text, conflicting source evidence, 64-draft overflow,
    /// missing articles and SQLite errors. Existing recovery data is preserved.
    pub fn save_note_draft(&self, draft: &NoteDraft) -> Result<()> {
        let target = encode_target(draft)?;
        let key = draft.target.key();
        let transaction = self.write_transaction()?;
        let previous: Option<String> = transaction
            .query_row(
                "SELECT target FROM note_drafts WHERE key = ?1",
                [&key],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            ensure!(
                previous == target,
                "Recover the earlier source-bound draft first"
            );
        } else {
            let count: usize = transaction.query_row(
                "SELECT count(*) FROM note_drafts",
                [],
                |row| row.get(0),
            )?;
            ensure!(
                count < 64,
                "Recover or discard a draft before creating another"
            );
        }
        self.article(draft.target.article())?;
        transaction.execute(
            include_str!("../sql/notebook/insert_draft.sql"),
            (
                &key,
                target,
                &draft.body,
                &draft.baseline,
                draft.initial_kind.map(|kind| kind.name()),
            ),
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Check a captured recovery slot without loading its private text.
    ///
    /// # Errors
    /// Returns SQLite failures.
    pub fn has_note_draft(&self, target: &DraftTarget) -> Result<bool> {
        Ok(self.connection.query_row(
            "SELECT EXISTS (SELECT 1 FROM note_drafts WHERE key = ?1)",
            [target.key()],
            |row| row.get(0),
        )?)
    }

    /// Count recoverable checkpoints without loading their text at startup.
    ///
    /// # Errors
    /// Returns SQLite failures.
    pub fn note_draft_count(&self) -> Result<usize> {
        Ok(self.connection.query_row(
            "SELECT count(*) FROM note_drafts",
            [],
            |row| row.get(0),
        )?)
    }

    /// Read at most 64 saved draft snapshots, without deleting any checkpoint.
    ///
    /// # Errors
    /// Returns invalid stored targets or SQLite errors.
    pub fn note_drafts(&self) -> Result<Vec<NoteDraft>> {
        let rows = self
            .connection
            .prepare_cached(include_str!("../sql/notebook/select_drafts.sql"))?
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(target, body, baseline, initial_kind)| {
                Ok(NoteDraft {
                    target: serde_json::from_str(&target)?,
                    body,
                    baseline,
                    initial_kind: initial_kind
                        .as_deref()
                        .map(crate::model::NoteKind::named)
                        .transpose()?,
                })
            })
            .collect()
    }

    /// Explicitly discard one captured recovery checkpoint.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn discard_note_draft(&self, target: &DraftTarget) -> Result<()> {
        self.connection.execute(
            "DELETE FROM note_drafts WHERE key = ?1",
            [target.key()],
        )?;
        Ok(())
    }
}

pub(crate) fn clear_written_draft(
    transaction: &Transaction<'_>,
    target: &DraftTarget,
    text: &str,
) -> Result<()> {
    transaction.execute(
        "DELETE FROM note_drafts WHERE key = ?1 AND body = ?2",
        (target.key(), text),
    )?;
    Ok(())
}

fn validate_text(text: &str) -> Result<()> {
    ensure!(text.len() <= 16384, "Draft exceeds 16 KiB");
    ensure!(
        text.chars().all(|character| !character.is_control()
            || matches!(character, '\n' | '\t')),
        "Draft contains terminal control characters"
    );
    Ok(())
}

fn encode_target(draft: &NoteDraft) -> Result<String> {
    validate_text(&draft.body)?;
    validate_text(&draft.baseline)?;
    let target = serde_json::to_string(&draft.target)?;
    ensure!(target.len() <= 32768, "Draft source exceeds 32 KiB");
    Ok(target)
}
