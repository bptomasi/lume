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
                    enabled: row.get(3)?,
                    error: row.get(4)?,
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
        ensure!(candidates.len() <= 5000, "Server exceeds 5,000 feeds");
        let mut remote_ids = BTreeSet::new();
        let transaction = self.connection.transaction()?;
        transaction.execute(
            include_str!("sql/servers/archive_feeds.sql"),
            [server_id],
        )?;
        for candidate in candidates {
            upsert_server_feed(
                &transaction,
                server_id,
                candidate,
                &mut remote_ids,
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    /// Ingest one remote batch, seeding flags only for first-seen articles.
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
        ensure!(articles.len() <= 2000, "Feed exceeds 2,000 entries");
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
        let transaction = self.connection.transaction()?;
        transaction.execute(
            include_str!("sql/servers/update_feed_title.sql"),
            (title, feed.id),
        )?;
        for article in articles {
            ingest_remote_article(&transaction, feed, article, remote)?;
        }
        transaction.execute(
            include_str!("sql/servers/update_cursor.sql"),
            (
                remote.server_id,
                remote.remote_id,
                remote.last_changed,
                remote.sync_offset,
                remote.sync_latest,
            ),
        )?;
        transaction.commit()?;
        Ok(())
    }
}

fn ingest_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    feed: &Feed,
    article: &IncomingArticle,
    remote: &RemoteFeedUpdate,
) -> Result<()> {
    let remote_id = article
        .external_id
        .strip_prefix("miniflux:")
        .and_then(|id| id.parse::<i64>().ok())
        .ok_or_else(|| {
            anyhow::anyhow!("Remote article has an invalid identity")
        })?;
    ensure!(remote_id > 0, "Remote article needs a positive ID");
    let article_id =
        resolve_remote_article(transaction, feed, article, remote, remote_id)?;
    transaction.execute(
        include_str!("sql/servers/insert_entry_mapping.sql"),
        (remote.server_id, remote_id, article_id),
    )?;
    Ok(())
}

fn resolve_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    feed: &Feed,
    article: &IncomingArticle,
    remote: &RemoteFeedUpdate,
    remote_id: i64,
) -> Result<i64> {
    if let Some(id) = mapped_article(transaction, remote.server_id, remote_id)?
    {
        update_remote_article(transaction, id, article)?;
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
    let state = remote.states.get(&article.external_id).copied().unwrap_or(
        crate::model::RemoteArticleState {
            unread: true,
            starred: false,
        },
    );
    transaction.execute(
        include_str!("sql/insert_remote_article.sql"),
        params![
            feed.id,
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
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn mapped_article(
    transaction: &rusqlite::Transaction<'_>,
    server_id: i64,
    remote_id: i64,
) -> Result<Option<i64>> {
    Ok(transaction
        .query_row(
            include_str!("sql/servers/select_entry_mapping.sql"),
            (server_id, remote_id),
            |row| row.get(0),
        )
        .optional()?)
}

fn article_by_external(
    transaction: &rusqlite::Transaction<'_>,
    feed_id: i64,
    external_id: &str,
) -> Result<Option<i64>> {
    Ok(transaction
        .query_row(
            include_str!("sql/servers/select_article_external.sql"),
            (feed_id, external_id),
            |row| row.get(0),
        )
        .optional()?)
}

fn article_by_canonical(
    transaction: &rusqlite::Transaction<'_>,
    feed_id: i64,
    canonical: &str,
) -> Result<Option<i64>> {
    Ok(transaction
        .query_row(
            include_str!("sql/select_article_by_canonical.sql"),
            (feed_id, canonical),
            |row| row.get(0),
        )
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
    transaction.execute(
        include_str!("sql/insert_server_feed.sql"),
        params![
            server_id,
            candidate.remote_id,
            candidate.remote_url,
            feed_id
        ],
    )?;
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
        .query_row(
            include_str!("sql/servers/select_feed_identity.sql"),
            (server_id, remote_id),
            |row| row.get(0),
        )
        .optional()?;
    if let Some(id) = existing {
        transaction.execute(
            include_str!("sql/servers/update_feed_identity.sql"),
            (id, &local_url, title),
        )?;
        Ok(id)
    } else {
        transaction.execute(
            include_str!("sql/upsert_server_feed.sql"),
            params![local_url, title, server_id],
        )?;
        Ok(transaction.query_row(
            include_str!("sql/servers/select_feed_id.sql"),
            [&local_url],
            |row| row.get(0),
        )?)
    }
}

fn update_remote_article(
    transaction: &rusqlite::Transaction<'_>,
    id: i64,
    article: &IncomingArticle,
) -> Result<()> {
    transaction.execute(
        include_str!("sql/update_remote_article.sql"),
        params![
            id,
            article.title,
            article.url,
            article.author,
            article.published,
            article.body,
            article.body_origin.as_str(),
            crate::content::reading_minutes(&article.body) as i64,
            crate::source::canonical_article_url(&article.url),
        ],
    )?;
    Ok(())
}

mod manage;

#[cfg(test)]
mod tests;
