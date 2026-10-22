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
                r#"
                UPDATE servers
                SET attempted_at = ?1, succeeded_at = ?1, error = ''
                WHERE id = ?2
                "#,
                (now, id),
            )?;
        } else {
            self.connection.execute(
                r#"
                UPDATE servers
                SET attempted_at = ?1, error = ?2
                WHERE id = ?3
                "#,
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
            "UPDATE server_feeds SET present = 0 WHERE server_id = ?1",
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
            "UPDATE feeds SET title = ?1 WHERE id = ?2 AND title <> ?1",
            (title, feed.id),
        )?;
        for article in articles {
            ingest_remote_article(&transaction, feed, article, remote)?;
        }
        transaction.execute(
            r#"
            UPDATE server_feeds
            SET initialized = 1,
                last_changed = max(last_changed, ?3)
            WHERE server_id = ?1 AND remote_id = ?2
            "#,
            (remote.server_id, remote.remote_id, remote.last_changed),
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
    let article_id =
        resolve_remote_article(transaction, feed, article, remote, remote_id)?;
    transaction.execute(
        r#"
        INSERT OR IGNORE INTO server_entries (
            server_id, remote_id, article_id
        ) VALUES (?1, ?2, ?3)
        "#,
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
            r#"
            SELECT article_id FROM server_entries
            WHERE server_id = ?1 AND remote_id = ?2
            "#,
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
            r#"
            SELECT id FROM articles
            WHERE feed_id = ?1 AND external_id = ?2
            "#,
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
    transaction.execute(
        include_str!("sql/upsert_server_feed.sql"),
        params![local_url, title, server_id],
    )?;
    let feed_id: i64 = transaction.query_row(
        "SELECT id FROM feeds WHERE url = ?1",
        [local_url.as_str()],
        |row| row.get(0),
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::Result;

    use crate::model::{
        FeedOrigin, IncomingArticle, RemoteArticleState, RemoteFeedCandidate,
        RemoteFeedUpdate,
    };
    use crate::store::Store;

    struct Fixture {
        _directory: tempfile::TempDir,
        store: Store,
        feed: crate::model::Feed,
        server_id: i64,
    }

    fn mirrored_store() -> Result<Fixture> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let mut store = Store::open(&path)?;
        store.connection.execute(
            "INSERT INTO servers (url, token_env) VALUES (?1, ?2)",
            ("https://reader.example", "LUME_MINIFLUX_TOKEN"),
        )?;
        let server_id = store.connection.last_insert_rowid();
        store.sync_server_feeds(
            server_id,
            &[RemoteFeedCandidate {
                remote_id: 7,
                title: "Remote journal".into(),
                remote_url: "https://publisher.example/feed".into(),
                local_url: "https://reader.example/v1/feeds/7".into(),
            }],
        )?;
        let feed = store
            .feeds()?
            .into_iter()
            .find(|feed| feed.server.is_some())
            .ok_or_else(|| anyhow::anyhow!("Missing mirrored feed"))?;
        Ok(Fixture {
            _directory: directory,
            store,
            feed,
            server_id,
        })
    }

    #[test]
    fn remote_refresh_preserves_local_state() -> Result<()> {
        let mut fixture = mirrored_store()?;
        let store = &mut fixture.store;
        let feed = &fixture.feed;
        store.ingest_remote(
            feed,
            "Remote journal",
            &[article("miniflux:1", "First")],
            &update(
                fixture.server_id,
                RemoteArticleState {
                    unread: false,
                    starred: true,
                },
                100,
            ),
        )?;
        let first = store.library()?.articles.remove(0);
        assert!(!first.unread);
        assert!(first.starred);
        store.set_unread(first.id, true)?;
        store.set_starred(first.id, false)?;
        store.ingest_remote(
            feed,
            "Remote journal",
            &[article("miniflux:1", "Revised")],
            &update(
                fixture.server_id,
                RemoteArticleState {
                    unread: false,
                    starred: true,
                },
                200,
            ),
        )?;
        let articles = store.library()?.articles;
        assert_eq!(articles.len(), 1);
        assert_eq!(articles[0].title, "Revised");
        assert!(articles[0].unread);
        assert!(!articles[0].starred);
        Ok(())
    }

    #[test]
    fn server_feeds_are_omitted_from_portable_feeds() -> Result<()> {
        let mut fixture = mirrored_store()?;
        fixture.store.sync_server_feeds(fixture.server_id, &[])?;
        let feed = fixture
            .store
            .feeds()?
            .into_iter()
            .find(|feed| feed.server.is_some())
            .ok_or_else(|| anyhow::anyhow!("Missing mirrored feed"))?;
        let server = feed
            .server
            .ok_or_else(|| anyhow::anyhow!("Missing server state"))?;
        assert!(!server.present);
        assert!(fixture.store.capture_configuration()?.feeds.is_empty());
        Ok(())
    }

    fn article(external_id: &str, title: &str) -> IncomingArticle {
        IncomingArticle {
            external_id: external_id.into(),
            title: title.into(),
            url: "https://publisher.example/article".into(),
            author: "Author".into(),
            published: 1_800_000_000,
            body: format!("{title} body"),
            body_origin: FeedOrigin::Feed,
        }
    }

    fn update(
        server_id: i64,
        state: RemoteArticleState,
        last_changed: i64,
    ) -> RemoteFeedUpdate {
        RemoteFeedUpdate {
            server_id,
            remote_id: 7,
            initialized: true,
            last_changed,
            states: BTreeMap::from([("miniflux:1".into(), state)]),
        }
    }
}
