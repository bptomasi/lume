//! Explicit historical restoration and source repair retain an undo snapshot.

use anyhow::{Context, Result, ensure};

use crate::{
    model::{Comment, ContentSource, NoteVersion, Passage},
    store::{
        Store,
        history::{self, Undo},
    },
};

impl Store {
    /// Return the latest 100 versions for an active, owned note.
    ///
    /// # Errors
    /// Returns ownership, Trash or SQLite errors.
    pub fn note_versions(
        &self,
        article: i64,
        note: i64,
    ) -> Result<Vec<NoteVersion>> {
        self.note_summary(note)?;
        self.annotation(article, note)?;
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "../sql/notebook/select_versions.sql"
            ))?
            .query_map([note], |row| {
                Ok(NoteVersion {
                    id: row.get(0)?,
                    article: row.get(1)?,
                    comment: Comment {
                        id: row.get(2)?,
                        body: row.get(3)?,
                        quote: row.get(4)?,
                        anchor: row.get(5)?,
                        source: if row.get::<_, String>(6)? == "full" {
                            ContentSource::Full
                        } else {
                            ContentSource::Feed
                        },
                        source_revision: row.get(7)?,
                        created: row.get(8)?,
                    },
                    saved_at: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Restore a captured version only if the currently previewed note is intact.
    ///
    /// # Errors
    /// Rejects changed/deleted notes, wrong versions, references or SQLite errors.
    pub fn restore_note_version(
        &self,
        article: i64,
        current: &Comment,
        version: i64,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        let before = self.annotation(article, current.id)?;
        ensure!(
            before.comment == *current,
            "Note changed; reopen its version preview"
        );
        let saved = self
            .note_versions(article, current.id)?
            .into_iter()
            .find(|item| item.id == version)
            .context("Version no longer belongs to this note")?;
        self.replace_note_content(article, &saved.comment)?;
        super::replace_references(
            &transaction,
            current.id,
            &saved.comment.body,
        )?;
        history::record(&transaction, &Undo::Annotation(Box::new(before)))?;
        transaction.commit()?;
        Ok(())
    }

    /// Replace an anchor after the user confirms a new selected source passage.
    ///
    /// # Errors
    /// Rejects stale notes/sources, invalid passages, Trash or SQLite errors.
    pub fn reanchor_note(
        &self,
        article: i64,
        current: &Comment,
        passage: &Passage,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        let before = self.annotation(article, current.id)?;
        ensure!(!self.is_deleted(article)?, "Restore the article first");
        ensure!(
            before.comment == *current,
            "Note changed; reopen its anchor preview"
        );
        super::super::comments::validate_source(
            &transaction,
            article,
            passage,
        )?;
        if passage.source == ContentSource::Feed {
            ensure!(
                self.body(article)?
                    .get(passage.anchor..)
                    .is_some_and(|tail| tail.starts_with(&passage.quote)),
                "Source changed; select the passage again"
            );
        }
        super::super::comments::validate(&current.body, &passage.quote)?;
        ensure!(!passage.quote.trim().is_empty(), "Select a passage first");
        let mut updated = current.clone();
        updated.quote.clone_from(&passage.quote);
        updated.anchor = passage.anchor;
        updated.source = passage.source;
        updated.source_revision = passage.revision;
        self.replace_note_content(article, &updated)?;
        history::record(&transaction, &Undo::Annotation(Box::new(before)))?;
        transaction.commit()?;
        Ok(())
    }

    fn replace_note_content(&self, article: i64, note: &Comment) -> Result<()> {
        let changed = self.connection.execute(
            include_str!("../sql/notebook/update_note_content.sql"),
            (
                &note.body,
                &note.quote,
                i64::try_from(note.anchor)?,
                note.source.as_str(),
                note.source_revision,
                note.id,
                article,
            ),
        )?;
        ensure!(changed == 1, "Note no longer belongs to this article");
        Ok(())
    }
}
