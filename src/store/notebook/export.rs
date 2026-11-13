//! Per-note export ownership uses permanent IDs even after deletion and undo.

use crate::store::Store;
use anyhow::{Context, Result, ensure};
use rusqlite::OptionalExtension;

impl Store {
    /// Read annotation export data atomically, excluding the source document.
    ///
    /// # Errors
    /// Rejects missing/trashed notes or SQLite failures.
    pub fn note_export(&self, id: i64) -> Result<crate::model::NoteExport> {
        let transaction = self.connection.unchecked_transaction()?;
        // Export needs ownership and citation, not a preview or article counts.
        let (article, url): (i64, String) = transaction
            .query_row(
                include_str!("../sql/notebook/select_export_source.sql"),
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .context("Note is missing or its article is in Trash")?;
        let note = self.annotation(article, id)?;
        let backlinks = self.note_backlinks(id)?;
        transaction.commit()?;
        Ok(crate::model::NoteExport {
            note,
            url,
            backlinks,
        })
    }

    /// Read the last owned bytes of a stable Note-ID.md file.
    ///
    /// # Errors
    /// Returns SQLite failures.
    pub fn exported_note(
        &self,
        namespace: &str,
        note: i64,
    ) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                include_str!("../sql/notebook/select_export_manifest.sql"),
                (namespace, note),
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Record exact file bytes after successful publication.
    ///
    /// # Errors
    /// Rejects content over 128 KiB or SQLite failures.
    pub fn record_note_export(
        &self,
        namespace: &str,
        note: i64,
        content: &str,
    ) -> Result<()> {
        ensure!(content.len() <= 131072, "Note export exceeds 128 KiB");
        self.connection.execute(
            include_str!("../sql/notebook/insert_export_manifest.sql"),
            (namespace, note, content),
        )?;
        Ok(())
    }
}
