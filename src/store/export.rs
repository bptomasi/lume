//! Exact-byte ownership manifests for incremental Markdown reconciliation.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use super::Store;

impl Store {
    /// Load the last successfully published bytes for one managed file.
    ///
    /// # Errors
    /// Returns SQLite errors; a new namespace/article has no previous content.
    pub fn exported_content(
        &self,
        namespace: &str,
        article: i64,
    ) -> Result<Option<String>> {
        Ok(self
            .connection
            .query_row(
                r#"
                SELECT content
                FROM export_manifest
                WHERE namespace = ?1
                    AND article_id = ?2
                "#,
                (namespace, article),
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Record ownership only after the file's complete bytes were published.
    ///
    /// # Errors
    /// Returns excessive content or SQLite errors; files are never removed.
    pub fn record_export(
        &self,
        namespace: &str,
        article: i64,
        content: &str,
    ) -> Result<()> {
        ensure!(content.len() <= 1024 * 1024, "Managed export exceeds 1 MiB");
        self.connection.execute(
            r#"
            INSERT INTO export_manifest (namespace, article_id, content)
            VALUES (?1, ?2, ?3)
            ON CONFLICT (namespace, article_id) DO UPDATE SET
                content = excluded.content
            "#,
            (namespace, article, content),
        )?;
        Ok(())
    }
}
