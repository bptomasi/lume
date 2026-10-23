//! Explicit cache reclamation preserves publisher feed text and personal context.
//!
//! Emptying Full bodies fires the existing FTS/recommendation triggers; deleting
//! cache rows would leave the Full text in FTS on older schemas. SQLite can
//! reuse freed pages; this operation does not promise to shrink the database.

use anyhow::Result;

use super::Store;

impl Store {
    pub(crate) fn storage_usage(
        &self,
    ) -> Result<crate::storage::report::Usage> {
        let mut usage = self.connection.query_row(
            include_str!("sql/select_storage_usage.sql"),
            [],
            |row| {
                Ok(crate::storage::report::Usage {
                    articles: row.get(0)?,
                    feed: row.get(1)?,
                    full: row.get(2)?,
                    notes: row.get(3)?,
                    comments: row.get(4)?,
                    highlights: row.get(5)?,
                    ..Default::default()
                })
            },
        )?;
        let pages: u64 =
            self.connection
                .query_row("PRAGMA page_count", [], |row| row.get(0))?;
        let free: u64 =
            self.connection
                .query_row("PRAGMA freelist_count", [], |row| row.get(0))?;
        let page_size: u64 =
            self.connection
                .query_row("PRAGMA page_size", [], |row| row.get(0))?;
        usage.pages = pages.saturating_mul(page_size);
        usage.free = free.saturating_mul(page_size);
        Ok(usage)
    }

    pub(crate) fn cleanup_boundary(&self) -> Result<i64> {
        Ok(self.connection.query_row(
            "SELECT coalesce(max(article_id), 0) FROM article_documents",
            [],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn clear_document_batch(
        &self,
        after: i64,
        upper: i64,
    ) -> Result<(usize, u64, i64)> {
        let transaction = self.write_transaction()?;
        let protected = include_str!("sql/retention/select_protected.sql")
            .trim()
            .trim_end_matches(';');
        let query = format!(
            "WITH protected AS MATERIALIZED ({protected})\n{}",
            include_str!("sql/select_disposable_documents.sql")
        );
        let entries = transaction
            .prepare_cached(&query)?
            .query_map((after, upper), |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, u64>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut bytes = 0;
        for (article, size, body) in &entries {
            transaction.execute(
                include_str!("sql/update_disposable_document.sql"),
                [article],
            )?;
            transaction.execute(
                include_str!("sql/retention/update_reading_minutes.sql"),
                (crate::content::reading_minutes(body), article),
            )?;
            bytes += size;
        }
        transaction.commit()?;
        Ok((
            entries.len(),
            bytes,
            entries.last().map_or(after, |entry| entry.0),
        ))
    }

    pub(crate) fn visit_cached_bodies(
        &self,
        cancel: &std::sync::atomic::AtomicBool,
        mut visit: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let mut statement = self
            .connection
            .prepare(include_str!("sql/select_cached_bodies.sql"))?;
        let mut rows = statement.query([])?;
        while !cancel.load(std::sync::atomic::Ordering::Relaxed) {
            let Some(row) = rows.next()? else { break };
            let body: String = row.get(0)?;
            visit(&body)?;
        }
        Ok(())
    }
}
