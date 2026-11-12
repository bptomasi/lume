//! Occurrence browsing preserves each publisher entry and its community links.

use anyhow::Result;

use super::{Store, article_row};

impl Store {
    /// Return up to 500 active occurrences sharing an exact normalized URL.
    ///
    /// # Errors
    /// Returns missing article or SQLite errors; invalid URLs group nothing.
    pub fn article_occurrences(
        &self,
        article: i64,
    ) -> Result<Vec<crate::model::Article>> {
        let key: String = self.connection.query_row(
            include_str!("sql/occurrences/select_canonical_key.sql"),
            [article],
            |row| row.get(0),
        )?;
        if key.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            include_str!("sql/occurrences/template_occurrences.sql"),
            include_str!("sql/select_article_metadata.sql")
        );
        Ok(self
            .connection
            .prepare_cached(&sql)?
            .query_map([key], article_row)?
            .collect::<rusqlite::Result<_>>()?)
    }
}
