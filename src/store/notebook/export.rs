//! Per-note export ownership uses permanent IDs even after deletion and undo.

use crate::store::Store;
use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

impl Store {
    /// Read annotation export data atomically, excluding the source document.
    ///
    /// # Errors
    /// Rejects missing/trashed notes or SQLite failures.
    pub fn note_export(&self, id: i64) -> Result<crate::model::NoteExport> {
        let transaction = self.connection.unchecked_transaction()?;
        let summary = self.note_summary(id)?;
        let note = self.annotation(summary.article, id)?;
        let url = self.article(summary.article)?.url;
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
        Ok(self.connection.query_row(
            "SELECT content FROM note_export_manifest WHERE namespace = ?1 AND note_id = ?2",
            (namespace, note), |row| row.get(0),
        ).optional()?)
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
