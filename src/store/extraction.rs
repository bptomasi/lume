//! Rule confirmation is atomic with its preview cache; no unreviewed rule saves.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use crate::model::{FeedOrigin, RulePreview, SourceHealth};

use super::Store;

impl Store {
    /// Load unexpired server-directed hostname retry deadlines.
    pub(crate) fn domain_retries(
        &self,
        now: i64,
    ) -> Result<std::collections::BTreeMap<String, i64>> {
        let rows = self
            .connection
            .prepare_cached(include_str!("sql/select_domain_retries.sql"))?
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut deadlines = std::collections::BTreeMap::new();
        for (host, value) in rows {
            let deadline: i64 = value.parse()?;
            if deadline > now {
                deadlines.insert(host, deadline);
            }
        }
        Ok(deadlines)
    }

    /// Keep the longest server-directed cooldown, independently of a feed ID.
    pub(crate) fn record_domain_retry(
        &self,
        host: &str,
        deadline: i64,
        now: i64,
    ) -> Result<()> {
        if deadline <= now {
            return Ok(());
        }
        let transaction = self.write_transaction()?;
        let key = format!("collector_retry:{host}");
        let previous: Option<String> = transaction
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [&key],
                |row| row.get(0),
            )
            .optional()?;
        let previous = previous
            .map(|value| value.parse::<i64>())
            .transpose()?
            .unwrap_or(0);
        transaction.execute(
            include_str!("sql/insert_domain_retry.sql"),
            (key, deadline.max(previous).to_string()),
        )?;
        transaction.execute(
            include_str!("sql/delete_expired_domain_retries.sql"),
            [now],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Read one source's consecutive failures without loading all diagnostics.
    ///
    /// # Errors
    /// Returns missing subscription or SQLite errors.
    pub fn source_failures(&self, feed: i64) -> Result<u32> {
        Ok(self.connection.query_row(
            "SELECT failures FROM feeds WHERE id = ?1",
            [feed],
            |row| row.get(0),
        )?)
    }

    /// Read the parser-recorded origin; older content remains explicitly Feed.
    ///
    /// # Errors
    /// Returns missing article or invalid stored origin errors.
    pub fn feed_origin(&self, article: i64) -> Result<FeedOrigin> {
        let origin: String = self.connection.query_row(
            "SELECT body_origin FROM articles WHERE id = ?1",
            [article],
            |row| row.get(0),
        )?;
        Ok(match origin.as_str() {
            "feed" => FeedOrigin::Feed,
            "summary" => FeedOrigin::Summary,
            "missing" => FeedOrigin::Missing,
            _ => anyhow::bail!("Invalid stored feed origin"),
        })
    }

    /// Read a source's CSS override; an empty string selects Readability.
    ///
    /// # Errors
    /// Returns a missing subscription or SQLite error.
    pub fn article_selector(&self, feed: i64) -> Result<String> {
        Ok(self.connection.query_row(
            "SELECT article_selector FROM feeds WHERE id = ?1",
            [feed],
            |row| row.get(0),
        )?)
    }

    /// Save exactly the previewed rule/cache while its target and rule are current.
    ///
    /// # Errors
    /// Stale rules, URLs, Trash, invalid content or SQLite failures roll back both.
    pub fn confirm_article_rule(&self, preview: &RulePreview) -> Result<()> {
        crate::source::article::rules::validate_selector(&preview.selector)?;
        super::documents::validate_document(&preview.document)?;
        let transaction = self.write_transaction()?;
        let target = &preview.target;
        ensure!(
            super::documents::current_target(&transaction, target)?
                && self.article(target.article)?.feed_id == preview.feed,
            "Preview target changed; preview again"
        );
        let changed = transaction.execute(
            r#"
            UPDATE feeds SET article_selector = ?1
            WHERE id = ?2 AND article_selector = ?3
            "#,
            (&preview.selector, preview.feed, &preview.previous),
        )?;
        ensure!(changed == 1, "Source rule changed; preview again");
        super::documents::write_document(
            &transaction,
            target,
            &preview.document,
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Load source diagnostics, including server-directed hostname cooldowns.
    ///
    /// # Errors
    /// Returns SQLite, invalid stored URL or retry-deadline decoding errors.
    pub fn source_health(&self) -> Result<Vec<SourceHealth>> {
        let retries = self.domain_retries(chrono::Utc::now().timestamp())?;
        let mut health = self
            .connection
            .prepare_cached(include_str!("sql/select_source_health.sql"))?
            .query_map([], |row| {
                Ok(SourceHealth {
                    feed: row.get(0)?,
                    title: row.get(1)?,
                    attempted_at: row.get(2)?,
                    succeeded_at: row.get(3)?,
                    retry_at: row.get(4)?,
                    failures: row.get(5)?,
                    error: row.get(6)?,
                    selector: row.get(7)?,
                    url: row.get(8)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for source in &mut health {
            let url = url::Url::parse(&source.url)?;
            if let Some(retry) =
                url.host_str().and_then(|host| retries.get(host))
            {
                source.retry_at = source.retry_at.max(*retry);
            }
        }
        Ok(health)
    }

    /// Record one attempt outcome; Retry-After is an absolute Unix deadline.
    ///
    /// # Errors
    /// Returns missing subscription or SQLite failures.
    pub fn record_source_attempt(
        &self,
        feed: i64,
        now: i64,
        error: &str,
        retry_at: i64,
    ) -> Result<()> {
        let changed = self.connection.execute(
            include_str!("sql/update_source_health.sql"),
            (
                feed,
                now,
                crate::content::label(error, 2000),
                retry_at.max(now),
            ),
        )?;
        ensure!(changed == 1, "Subscription no longer exists");
        Ok(())
    }
}
