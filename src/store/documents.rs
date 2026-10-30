//! Independent full-article cache, with URL guards and monotonic revisions.
//!
//! Failure updates preserve the last successful body. A publisher URL change
//! invalidates its use without deleting either cache or private annotations.
//! Cache writes never join the personal undo journal or mutate feed content.

use anyhow::{Result, bail, ensure};
use rusqlite::{OptionalExtension, Transaction};

use crate::{
    content::{self, BodyKind},
    model::{ArticleTarget, CachedDocument, ExtractedDocument, FetchPolicy},
};

use super::Store;

impl Store {
    /// Load one article's successful cache and last attempt, if any.
    ///
    /// # Errors
    /// Returns SQLite/decoding errors; absence is not a failure.
    pub fn cached_document(
        &self,
        article: i64,
    ) -> Result<Option<CachedDocument>> {
        Ok(self
            .connection
            .query_row(
                include_str!("sql/select_article_document.sql"),
                [article],
                |row| {
                    Ok(CachedDocument {
                        url: row.get(0)?,
                        final_url: row.get(1)?,
                        body: row.get(2)?,
                        revision: row.get(3)?,
                        fetched_at: row.get(4)?,
                        attempt_url: row.get(5)?,
                        attempted_at: row.get(6)?,
                        error: row.get(7)?,
                        progress: row.get(8)?,
                    })
                },
            )
            .optional()?)
    }

    /// Commit sanitized article text only while the captured URL is current.
    ///
    /// # Errors
    /// Returns invalid body/address or SQLite errors. A stale/deleted target
    /// returns false without changing the cache or search index.
    pub fn cache_document(
        &self,
        target: &ArticleTarget,
        document: &ExtractedDocument,
    ) -> Result<bool> {
        validate_document(document)?;
        crate::source::link_url(&target.url)?;
        let transaction = self.write_transaction()?;
        if !current_target(&transaction, target)? {
            return Ok(false);
        }
        write_document(&transaction, target, document)?;
        transaction.commit()?;
        Ok(true)
    }

    /// Record a failed attempt without replacing an older successful result.
    ///
    /// # Errors
    /// Returns invalid addresses or SQLite errors; stale targets return false.
    pub fn document_failure(
        &self,
        target: &ArticleTarget,
        error: &str,
    ) -> Result<bool> {
        crate::source::link_url(&target.url)?;
        let transaction = self.write_transaction()?;
        if !current_target(&transaction, target)? {
            return Ok(false);
        }
        transaction.execute(
            include_str!("sql/insert_document_failure.sql"),
            (target.article, &target.url, content::label(error, 2000)),
        )?;
        transaction.commit()?;
        Ok(true)
    }

    /// Save a Full reading anchor only for the revision that was displayed.
    ///
    /// # Errors
    /// Returns integer conversion or SQLite errors. A replaced revision is
    /// intentionally untouched, so delayed saves cannot corrupt its progress.
    pub fn save_document_progress(
        &self,
        article: i64,
        revision: i64,
        anchor: usize,
    ) -> Result<()> {
        self.connection.execute(
            include_str!("sql/update_full_progress.sql"),
            (i64::try_from(anchor)?, article, revision),
        )?;
        Ok(())
    }

    /// Read a subscription's independent article-fetch policy.
    ///
    /// # Errors
    /// Returns a missing subscription or SQLite error.
    pub fn fetch_policy(&self, feed: i64) -> Result<FetchPolicy> {
        let value: String = self.connection.query_row(
            "SELECT article_policy FROM feeds WHERE id = ?1",
            [feed],
            |row| row.get(0),
        )?;
        Ok(match value.as_str() {
            "always" => FetchPolicy::Always,
            "manual" => FetchPolicy::Manual,
            "missing" => FetchPolicy::Missing,
            _ => bail!("Unsupported article fetch policy"),
        })
    }

    /// Set policy without altering cached articles or feed HTTP validators.
    ///
    /// # Errors
    /// Returns a missing subscription or SQLite error.
    pub fn set_fetch_policy(
        &self,
        feed: i64,
        policy: FetchPolicy,
    ) -> Result<()> {
        let changed = self.connection.execute(
            "UPDATE feeds SET article_policy = ?1 WHERE id = ?2",
            (policy.as_str(), feed),
        )?;
        ensure!(changed == 1, "Subscription no longer exists");
        Ok(())
    }
}

pub(super) fn current_target(
    transaction: &Transaction<'_>,
    target: &ArticleTarget,
) -> Result<bool> {
    Ok(transaction.query_row(
        include_str!("sql/select_document_target.sql"),
        (target.article, &target.url),
        |row| row.get(0),
    )?)
}

pub(super) fn validate_document(document: &ExtractedDocument) -> Result<()> {
    ensure!(document.body.len() <= 524_288, "Article exceeds 512 KiB");
    ensure!(
        content::body_kind(&document.body) == BodyKind::Text,
        "No readable article text was extracted"
    );
    ensure!(
        !document
            .body
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\n' | '\t')),
        "Article contains terminal control characters"
    );
    crate::source::link_url(&document.final_url)?;
    crate::source::article::rules::reject_obstruction(&document.body)?;
    Ok(())
}

// Cache text, FTS triggers and the duration filter observe one transaction.
pub(super) fn write_document(
    transaction: &Transaction<'_>,
    target: &ArticleTarget,
    document: &ExtractedDocument,
) -> Result<()> {
    transaction.execute(
        include_str!("sql/insert_article_document.sql"),
        (
            target.article,
            &target.url,
            &document.final_url,
            &document.body,
        ),
    )?;
    transaction.execute(
        include_str!("sql/update_reading_minutes.sql"),
        (
            crate::content::reading_minutes(&document.body) as i64,
            target.article,
        ),
    )?;
    Ok(())
}
