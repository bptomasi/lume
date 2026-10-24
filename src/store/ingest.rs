//! Subscription lifecycle and atomic publisher-content updates.

use anyhow::{Result, ensure};
use rusqlite::params;

use crate::model::{Feed, IncomingArticle};

use super::Store;

#[cfg(test)]
mod tests;

impl Store {
    /// Replace an exact old feed title without overwriting a publisher rename.
    ///
    /// # Errors
    /// Returns invalid URLs/titles or SQLite errors. Cached articles and
    /// subscription validators retain their identities and values.
    pub fn rename_feed_title_if_current(
        &self,
        address: &str,
        previous: &str,
        replacement: &str,
    ) -> Result<bool> {
        let url = crate::source::feed_url(address)?;
        ensure!(
            !replacement.trim().is_empty()
                && replacement.chars().count() <= 200
                && !replacement.chars().any(char::is_control),
            "Invalid feed title"
        );
        let changed = self.connection.execute(
            r#"
            UPDATE feeds
            SET title = ?1
            WHERE url = ?2 AND title = ?3
            "#,
            (replacement, url.as_str(), previous),
        )?;
        Ok(changed == 1)
    }

    /// Import already validated feed candidates as one atomic operation.
    ///
    /// # Errors
    /// Returns invalid URLs or SQLite errors without a partial import.
    pub fn import_feeds(
        &self,
        feeds: &[crate::source::discovery::Candidate],
    ) -> Result<usize> {
        ensure!(feeds.len() <= 5000, "Import exceeds 5,000 subscriptions");
        let validated = feeds
            .iter()
            .map(|feed| {
                Ok((
                    crate::source::feed_url(&feed.url)?.to_string(),
                    crate::content::label(&feed.title, 200),
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let transaction = self.connection.unchecked_transaction()?;
        let mut imported = 0;
        for (url, title) in validated {
            let title = if title.trim().is_empty() {
                url.as_str()
            } else {
                title.as_str()
            };
            imported += transaction.execute(
                r#"
                INSERT INTO feeds (url, title) VALUES (?1, ?2)
                ON CONFLICT (url) DO UPDATE SET enabled = 1
                WHERE feeds.enabled = 0
                "#,
                (&url, title),
            )?;
        }
        transaction.commit()?;
        Ok(imported)
    }

    /// Add a normalized feed URL, or re-enable an existing subscription.
    ///
    /// # Errors
    /// Returns invalid URL or SQLite errors.
    pub fn subscribe(&self, address: &str) -> Result<i64> {
        let url = crate::source::feed_url(address)?;
        self.connection.execute(
            r#"
            INSERT INTO feeds (url, title) VALUES (?1, ?1)
            ON CONFLICT (url) DO UPDATE SET enabled = 1
            "#,
            [url.as_str()],
        )?;
        Ok(self.connection.query_row(
            "SELECT id FROM feeds WHERE url = ?1",
            [url.as_str()],
            |row| row.get(0),
        )?)
    }

    /// Remove a subscription and its unsaved cached articles atomically.
    /// Personal folders, favorites, notes and other explicit saves survive.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn unsubscribe(&self, id: i64) -> Result<()> {
        let transaction = self.write_transaction()?;
        super::deletion::purge::source(
            &transaction,
            super::deletion::purge::Source::Feed(id),
        )?;
        transaction
            .execute(include_str!("sql/deletion/archive_feed.sql"), [id])?;
        transaction.commit()?;
        Ok(())
    }

    /// Return active subscriptions in stable display order.
    ///
    /// # Errors
    /// Returns SQLite query or decoding errors.
    pub fn feeds(&self) -> Result<Vec<Feed>> {
        Ok(self
            .connection
            .prepare(include_str!("sql/select_active_feeds.sql"))?
            .query_map([], feed_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn publisher_feeds(&self) -> Result<Vec<Feed>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/select_publisher_feeds.sql"))?
            .query_map([], feed_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    pub(crate) fn server_feeds(&self, server: i64) -> Result<Vec<Feed>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/select_server_feeds.sql"))?
            .query_map([server], feed_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Ingest one complete, bounded feed while preserving personal state.
    ///
    /// # Errors
    /// Returns validation or transaction errors; all changes roll back.
    pub fn ingest(
        &mut self,
        feed: &Feed,
        title: &str,
        articles: &[IncomingArticle],
    ) -> Result<()> {
        ensure!(articles.len() <= 2000, "Feed exceeds 2,000 entries");
        let transaction = self.write_transaction()?;
        {
            let mut insert = transaction
                .prepare_cached(include_str!("sql/insert_article.sql"))?;
            for article in articles {
                if unchanged_article(&transaction, feed.id, article)? {
                    continue;
                }
                insert.execute(params![
                    feed.id,
                    article.external_id,
                    article.title,
                    article.url,
                    article.author,
                    article.published,
                    article.body,
                    article.body_origin.as_str(),
                    crate::content::reading_minutes(&article.body) as i64,
                    crate::source::canonical_article_url(&article.url),
                ])?;
            }
        }
        transaction.execute(
            include_str!("sql/update_feed_validators.sql"),
            (&feed.etag, &feed.modified, feed.id),
        )?;
        // FTS tracks feed titles. Avoid rebuilding every article's index when
        // the publisher returns the same title on a routine refresh.
        transaction.execute(
            include_str!("sql/update_feed_title.sql"),
            (title, feed.id),
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Record a bounded collection failure or clear it after a 304 response.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn feed_error(&self, id: i64, error: &str) -> Result<()> {
        let message: String = error.chars().take(500).collect();
        self.connection.execute(
            include_str!("sql/update_feed_error.sql"),
            (message, id),
        )?;
        Ok(())
    }
}

fn unchanged_article(
    transaction: &rusqlite::Transaction<'_>,
    feed: i64,
    article: &IncomingArticle,
) -> Result<bool> {
    Ok(transaction
        .prepare_cached(include_str!("sql/select_unchanged_article.sql"))?
        .query_row(
            params![
                feed,
                article.external_id,
                article.title,
                article.url,
                article.author,
                article.published,
                article.body,
                article.body_origin.as_str(),
            ],
            |row| row.get(0),
        )?)
}

fn feed_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Feed> {
    let server_id: Option<i64> = row.get(6)?;
    let server = match server_id {
        Some(server_id) => Some(crate::model::ServerFeed {
            server_id,
            server_url: row.get(7)?,
            token_env: row.get(8)?,
            token: row
                .get::<_, Option<String>>(16)?
                .filter(|token| !token.is_empty())
                .map(Into::into),
            enabled: row.get(9)?,
            remote_id: row.get(10)?,
            initialized: row.get(11)?,
            last_changed: row.get(12)?,
            present: row.get(13)?,
            sync_after_id: row.get(14)?,
            sync_latest: row.get(15)?,
            sync: server_sync_row(row, 17)?,
            history: crate::model::ServerHistory {
                before_id: row.get(18)?,
                loaded: row.get(19)?,
                total: row.get(20)?,
            },
        }),
        None => None,
    };
    Ok(Feed {
        id: row.get(0)?,
        title: row.get(1)?,
        url: row.get(2)?,
        etag: row.get(3)?,
        modified: row.get(4)?,
        error: row.get(5)?,
        server,
    })
}

pub(super) fn server_sync_row(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> rusqlite::Result<crate::model::ServerSync> {
    serde_json::from_str(&row.get::<_, String>(index)?).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(
            index,
            rusqlite::types::Type::Text,
            Box::new(error),
        )
    })
}
