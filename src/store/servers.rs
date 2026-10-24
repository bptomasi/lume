//! Optional server projections and first-observation article mapping.
//!
//! Remote content is cached like publisher content. Personal flags change only
//! when an article is first observed; later refreshes never overwrite them.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use rusqlite::{OptionalExtension, params};

use crate::{
    model::{
        Feed, IncomingArticle, RemoteFeedCandidate, RemoteFeedUpdate, Server,
    },
    store::Store,
};

impl Store {
    /// List configured servers, including disabled definitions.
    ///
    /// # Errors
    /// Returns SQLite query or decoding errors.
    pub fn servers(&self) -> Result<Vec<Server>> {
        Ok(self
            .connection
            .prepare_cached(include_str!("sql/select_servers.sql"))?
            .query_map([], |row| {
                Ok(Server {
                    id: row.get(0)?,
                    url: row.get(1)?,
                    token_env: row.get(2)?,
                    token: row
                        .get::<_, Option<String>>(5)?
                        .filter(|token| !token.is_empty())
                        .map(Into::into),
                    enabled: row.get(3)?,
                    error: row.get(4)?,
                    sync: super::ingest::server_sync_row(row, 6)?,
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Record one server-level attempt, preserving a bounded error message.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn record_server_attempt(
        &self,
        id: i64,
        now: i64,
        error: &str,
    ) -> Result<()> {
        let message: String = error.chars().take(500).collect();
        if message.is_empty() {
            self.connection.execute(
                include_str!("sql/servers/update_attempt_success.sql"),
                (now, id),
            )?;
        } else {
            self.connection.execute(
                include_str!("sql/servers/update_attempt_failure.sql"),
                (now, message, id),
            )?;
        }
        Ok(())
    }

    /// Replace the visible remote subscription set for one server.
    ///
    /// Missing feeds are retained with `present = 0`; their cached articles
    /// and personal state remain untouched.
    ///
    /// # Errors
    /// Returns invalid candidate data or SQLite errors; all rows roll back.
    pub fn sync_server_feeds(
        &mut self,
        server_id: i64,
        candidates: &[RemoteFeedCandidate],
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        persist_discovery(&transaction, server_id, candidates)?;
        transaction.commit()?;
        Ok(())
    }

    /// Publish discovered subscriptions and health against the captured connection.
    ///
    /// # Errors
    /// Returns invalid candidates or atomic storage failures; obsolete results return false.
    pub fn sync_collected_server_feeds(
        &mut self,
        server: &Server,
        candidates: &[RemoteFeedCandidate],
        now: i64,
    ) -> Result<bool> {
        let transaction = self.write_transaction()?;
        if !cursor::server_current(&transaction, server)? {
            return Ok(false);
        }
        persist_discovery(&transaction, server.id, candidates)?;
        self.record_server_attempt(server.id, now, "")?;
        transaction.commit()?;
        Ok(true)
    }

    pub(crate) fn record_collected_server_failure(
        &self,
        server: &Server,
        now: i64,
        error: &str,
    ) -> Result<bool> {
        let transaction = self.write_transaction()?;
        if !cursor::server_current(&transaction, server)? {
            return Ok(false);
        }
        self.record_server_attempt(server.id, now, error)?;
        transaction.commit()?;
        Ok(true)
    }

    /// Import a trusted remote batch, seeding flags only for first-seen articles.
    /// Network collection uses `ingest_collected_remote` to reject obsolete pages.
    ///
    /// # Errors
    /// Returns invalid remote identities, oversized bodies or SQLite errors.
    pub fn ingest_remote(
        &mut self,
        feed: &Feed,
        title: &str,
        articles: &[IncomingArticle],
        remote: &RemoteFeedUpdate,
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        persist_remote(&transaction, feed, title, articles, remote)?;
        transaction.commit()?;
        Ok(())
    }

    /// Commit a downloaded page only if its connection/cursor remains current.
    /// Obsolete pages leave content, progress and source health untouched.
    ///
    /// # Errors
    /// Returns malformed batches or atomic storage failures.
    pub fn ingest_collected_remote(
        &mut self,
        feed: &Feed,
        title: &str,
        articles: &[IncomingArticle],
        remote: &RemoteFeedUpdate,
        now: i64,
    ) -> Result<bool> {
        let transaction = self.write_transaction()?;
        if !cursor::current(&transaction, feed)? {
            return Ok(false);
        }
        persist_remote(&transaction, feed, title, articles, remote)?;
        self.record_source_attempt(feed.id, now, "", 0)?;
        transaction.commit()?;
        Ok(true)
    }

    pub(crate) fn record_collection_failure(
        &self,
        feed: &Feed,
        now: i64,
        error: &str,
        retry_at: i64,
    ) -> Result<bool> {
        let transaction = self.write_transaction()?;
        if !cursor::current(&transaction, feed)? {
            return Ok(false);
        }
        let failures = self.source_failures(feed.id)?;
        let delay = 60_i64.saturating_mul(1_i64 << failures.min(10));
        self.record_source_attempt(
            feed.id,
            now,
            error,
            retry_at.max(now.saturating_add(delay)),
        )?;
        transaction.commit()?;
        Ok(true)
    }
}

fn persist_discovery(
    transaction: &rusqlite::Transaction<'_>,
    server_id: i64,
    candidates: &[RemoteFeedCandidate],
) -> Result<()> {
    ensure!(candidates.len() <= 5000, "Server exceeds 5,000 feeds");
    let mut remote_ids = BTreeSet::new();
    for candidate in candidates {
        upsert_server_feed(transaction, server_id, candidate, &mut remote_ids)?;
    }
    transaction
        .prepare_cached(include_str!("sql/servers/archive_feeds.sql"))?
        .execute((server_id, serde_json::to_string(&remote_ids)?))?;
    Ok(())
}

fn persist_remote(
    transaction: &rusqlite::Transaction<'_>,
    feed: &Feed,
    title: &str,
    articles: &[IncomingArticle],
    remote: &RemoteFeedUpdate,
) -> Result<()> {
    ensure!(articles.len() <= 2000, "Feed exceeds 2,000 entries");
    validate_remote_feed(feed, remote)?;
    transaction
        .prepare_cached(include_str!("sql/servers/update_feed_title.sql"))?
        .execute((title, feed.id))?;
    let removed: bool = transaction
        .prepare_cached(include_str!(
            "sql/retention/select_has_tombstones.sql"
        ))?
        .query_row([feed.id], |row| row.get(0))?;
    for article in articles {
        ingest_remote_article(transaction, feed, article, remote, removed)?;
    }
    cursor::update(transaction, remote)
}

fn validate_remote_feed(feed: &Feed, remote: &RemoteFeedUpdate) -> Result<()> {
    let server = feed
        .server
        .as_ref()
        .filter(|server| server.server_id == remote.server_id)
        .ok_or_else(|| {
            anyhow::anyhow!("Remote feed does not match its server")
        })?;
    ensure!(
        server.remote_id == remote.remote_id,
        "Remote feed identity changed during collection"
    );
    Ok(())
}

fn ingest_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    feed: &Feed,
    article: &IncomingArticle,
    remote: &RemoteFeedUpdate,
    has_removed: bool,
) -> Result<()> {
    let remote_id = article
        .external_id
        .strip_prefix("miniflux:")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| {
            anyhow::anyhow!("Remote article has an invalid identity")
        })?;
    ensure!(remote_id > 0, "Remote article needs a positive ID");
    if has_removed
        && super::retention::removed(
            transaction,
            feed.id,
            &article.external_id,
            &crate::source::canonical_article_url(&article.url),
        )?
    {
        return Ok(());
    }
    let article_id =
        resolve_remote_article(transaction, feed, article, remote, remote_id)?;
    transaction
        .prepare_cached(include_str!("sql/servers/insert_entry_mapping.sql"))?
        .execute((remote.server_id, remote_id, article_id))?;
    Ok(())
}

fn resolve_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    feed: &Feed,
    article: &IncomingArticle,
    remote: &RemoteFeedUpdate,
    remote_id: i64,
) -> Result<i64> {
    if let Some((id, unchanged)) =
        mapped_article(transaction, remote.server_id, remote_id, article)?
    {
        if !unchanged {
            update_remote_article(transaction, id, article)?;
        }
        return Ok(id);
    }
    if let Some(id) =
        article_by_external(transaction, feed.id, &article.external_id)?
    {
        update_remote_article(transaction, id, article)?;
        return Ok(id);
    }
    let canonical = crate::source::canonical_article_url(&article.url);
    if let Some(id) = article_by_canonical(transaction, feed.id, &canonical)? {
        update_remote_article(transaction, id, article)?;
        return Ok(id);
    }
    insert_remote_article(transaction, feed.id, article, remote, &canonical)
}

fn insert_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    feed_id: i64,
    article: &IncomingArticle,
    remote: &RemoteFeedUpdate,
    canonical: &str,
) -> Result<i64> {
    let state = remote.states.get(&article.external_id).copied().unwrap_or(
        crate::model::RemoteArticleState {
            unread: true,
            starred: false,
        },
    );
    transaction
        .prepare_cached(include_str!("sql/insert_remote_article.sql"))?
        .execute(params![
            feed_id,
            article.external_id,
            article.title,
            article.url,
            article.author,
            article.published,
            article.body,
            article.body_origin.as_str(),
            crate::content::reading_minutes(&article.body) as i64,
            state.unread,
            state.starred,
            canonical,
        ])?;
    Ok(transaction.last_insert_rowid())
}

fn mapped_article(
    transaction: &rusqlite::Transaction<'_>,
    server_id: i64,
    remote_id: i64,
    article: &IncomingArticle,
) -> Result<Option<(i64, bool)>> {
    Ok(transaction
        .prepare_cached(include_str!("sql/servers/select_cached_entry.sql"))?
        .query_row(
            params![
                server_id,
                remote_id,
                article.title,
                article.url,
                article.author,
                article.published,
                article.body,
                article.body_origin.as_str(),
            ],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?)
}

fn article_by_external(
    transaction: &rusqlite::Transaction<'_>,
    feed_id: i64,
    external_id: &str,
) -> Result<Option<i64>> {
    Ok(transaction
        .prepare_cached(include_str!(
            "sql/servers/select_article_external.sql"
        ))?
        .query_row((feed_id, external_id), |row| row.get(0))
        .optional()?)
}

fn article_by_canonical(
    transaction: &rusqlite::Transaction<'_>,
    feed_id: i64,
    canonical: &str,
) -> Result<Option<i64>> {
    Ok(transaction
        .prepare_cached(include_str!("sql/select_article_by_canonical.sql"))?
        .query_row((feed_id, canonical), |row| row.get(0))
        .optional()?)
}

fn upsert_server_feed(
    transaction: &rusqlite::Transaction<'_>,
    server_id: i64,
    candidate: &RemoteFeedCandidate,
    remote_ids: &mut BTreeSet<i64>,
) -> Result<()> {
    ensure!(candidate.remote_id > 0, "Remote feed needs a positive ID");
    ensure!(
        remote_ids.insert(candidate.remote_id),
        "Duplicate remote feed ID"
    );
    let local_url = crate::source::feed_url(&candidate.local_url)?.to_string();
    let title = crate::content::label(&candidate.title, 200);
    let title = if title.trim().is_empty() {
        local_url.as_str()
    } else {
        title.as_str()
    };
    let feed_id = server_feed_identity(
        transaction,
        server_id,
        candidate.remote_id,
        &local_url,
        title,
    )?;
    transaction
        .prepare_cached(include_str!("sql/insert_server_feed.sql"))?
        .execute(params![
            server_id,
            candidate.remote_id,
            candidate.remote_url,
            feed_id
        ])?;
    Ok(())
}

fn server_feed_identity(
    transaction: &rusqlite::Transaction<'_>,
    server_id: i64,
    remote_id: i64,
    local_url: &str,
    title: &str,
) -> Result<i64> {
    let existing: Option<i64> = transaction
        .prepare_cached(include_str!("sql/servers/select_feed_identity.sql"))?
        .query_row((server_id, remote_id), |row| row.get(0))
        .optional()?;
    if let Some(id) = existing {
        transaction
            .prepare_cached(include_str!(
                "sql/servers/update_feed_identity.sql"
            ))?
            .execute((id, &local_url, title))?;
        Ok(id)
    } else {
        transaction
            .prepare_cached(include_str!("sql/upsert_server_feed.sql"))?
            .execute(params![local_url, title, server_id])?;
        Ok(transaction
            .prepare_cached(include_str!("sql/servers/select_feed_id.sql"))?
            .query_row([&local_url], |row| row.get(0))?)
    }
}

fn update_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    id: i64,
    article: &IncomingArticle,
) -> Result<()> {
    transaction
        .prepare_cached(include_str!("sql/update_remote_article.sql"))?
        .execute(params![
            id,
            article.title,
            article.url,
            article.author,
            article.published,
            article.body,
            article.body_origin.as_str(),
            crate::content::reading_minutes(&article.body) as i64,
            crate::source::canonical_article_url(&article.url),
        ])?;
    Ok(())
}

mod cursor;
mod manage;

#[cfg(test)]
mod tests;
