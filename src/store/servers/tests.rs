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

impl Fixture {
    fn ingest_distinct(
        &mut self,
        titles: &[&str],
    ) -> Result<Vec<IncomingArticle>> {
        let entries = titles
            .iter()
            .enumerate()
            .map(|(index, title)| {
                let mut entry =
                    article(&format!("miniflux:{}", index + 1), title);
                entry.url =
                    format!("https://publisher.example/article/{index}");
                entry
            })
            .collect::<Vec<_>>();
        self.store.ingest_remote(
            &self.feed,
            "Journal",
            &entries,
            &update(
                self.server_id,
                RemoteArticleState {
                    unread: true,
                    starred: false,
                },
                100,
            ),
        )?;
        Ok(entries)
    }

    fn article_id(&self, title: &str) -> Result<i64> {
        self.store
            .library()?
            .articles
            .iter()
            .find(|item| item.title == title)
            .map(|item| item.id)
            .ok_or_else(|| anyhow::anyhow!("Missing fixture: {title}"))
    }

    fn expire_unread(&self, article: i64) -> Result<()> {
        self.store.connection.execute(
            include_str!("../sql/test_age_retention.sql"),
            (article, 1_000_000_000, 1_000_000_000),
        )?;
        self.store
            .save_retention_policy(&crate::config::retention::Retention {
                defaults: crate::config::retention::Policy {
                    unread_days: crate::config::retention::Age(Some(1)),
                    ..Default::default()
                },
                ..Default::default()
            })
    }
}

fn mirrored_store() -> Result<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let mut store = Store::open(&path)?;
    let server_id =
        store.add_server("https://reader.example", "LUME_MINIFLUX_TOKEN")?;
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
fn unchanged_remote_content_performs_no_database_writes() -> Result<()> {
    let mut fixture = mirrored_store()?;
    let remote = update(
        fixture.server_id,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        100,
    );
    let articles = [article("miniflux:1", "Original")];
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &articles,
        &remote,
    )?;
    let before = fixture.store.connection.total_changes();
    fixture.store.sync_server_feeds(
        fixture.server_id,
        &[RemoteFeedCandidate {
            remote_id: 7,
            title: fixture.feed.title.clone(),
            remote_url: "https://publisher.example/feed".into(),
            local_url: fixture.feed.url.clone(),
        }],
    )?;
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &articles,
        &remote,
    )?;
    assert_eq!(fixture.store.connection.total_changes(), before);
    Ok(())
}

#[test]
fn changed_remote_bodies_replace_the_search_index() -> Result<()> {
    let mut fixture = mirrored_store()?;
    let remote = update(
        fixture.server_id,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        100,
    );
    let mut entry = article("miniflux:1", "Stable title");
    for term in ["sentinelfirst", "sentinelsecond"] {
        entry.body = term.into();
        fixture.store.ingest_remote(
            &fixture.feed,
            "Remote journal",
            std::slice::from_ref(&entry),
            &remote,
        )?;
        let query = crate::model::ArticleQuery {
            view: crate::model::View::All,
            text: format!("body:{term}"),
            offset: 0,
            limit: 10,
            retained: None,
        };
        assert_eq!(fixture.store.query_articles(&query)?.total, 1);
    }
    let query = crate::model::ArticleQuery {
        view: crate::model::View::All,
        text: "body:sentinelfirst".into(),
        offset: 0,
        limit: 10,
        retained: None,
    };
    assert_eq!(fixture.store.query_articles(&query)?.total, 0);
    Ok(())
}

#[test]
fn canonical_aliases_keep_both_remote_identities_after_url_changes()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let remote = update(
        fixture.server_id,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        100,
    );
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &[
            article("miniflux:1", "Original"),
            article("miniflux:2", "Alias"),
        ],
        &remote,
    )?;
    let saved = fixture.store.library()?.articles.remove(0);
    let note = fixture
        .store
        .save_comment(saved.id, None, "Keep alias note")?;
    let mut changed = article("miniflux:2", "Moved article");
    changed.url = "https://publisher.example/moved".into();
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &[changed],
        &remote,
    )?;
    let articles = fixture.store.library()?.articles;
    assert_eq!(articles.len(), 1);
    assert_eq!(articles[0].id, saved.id);
    assert_eq!(articles[0].title, "Moved article");
    assert_eq!(fixture.store.comments(saved.id)?[0].id, note);
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
        history: Default::default(),
        server_id,
        remote_id: 7,
        initialized: true,
        last_changed,
        sync_after_id: 0,
        sync_latest: last_changed,
        states: BTreeMap::from([("miniflux:1".into(), state)]),
    }
}

#[test]
fn changing_server_root_retains_feed_identity() -> Result<()> {
    let mut fixture = mirrored_store()?;
    let before = fixture.feed.id;
    let path = fixture._directory.path().join("config.toml");
    let session = crate::config::Session::open(&fixture.store, &path)?;
    let mut config = fixture.store.capture_configuration()?;
    config.servers[0].url = "https://other.example/base".into();
    std::fs::write(&path, crate::config::encode(&config)?)?;
    session.flush()?;
    fixture.store.sync_server_feeds(
        fixture.server_id,
        &[RemoteFeedCandidate {
            remote_id: 7,
            title: "Renamed journal".into(),
            remote_url: "https://publisher.example/feed".into(),
            local_url: "https://other.example/base/v1/feeds/7".into(),
        }],
    )?;
    let feeds = fixture.store.feeds()?;
    assert_eq!(feeds.len(), 1);
    assert_eq!(feeds[0].id, before);
    assert_eq!(feeds[0].url, "https://other.example/base/v1/feeds/7");
    Ok(())
}

#[test]
fn discovery_cannot_take_ownership_of_a_local_subscription() -> Result<()> {
    let mut fixture = mirrored_store()?;
    let local = fixture
        .store
        .subscribe("https://reader.example/v1/feeds/9")?;
    assert!(
        fixture
            .store
            .sync_server_feeds(
                fixture.server_id,
                &[RemoteFeedCandidate {
                    remote_id: 9,
                    title: "Collision".into(),
                    remote_url: "https://publisher.example/feed".into(),
                    local_url: "https://reader.example/v1/feeds/9".into(),
                }]
            )
            .is_err()
    );
    let feeds = fixture.store.feeds()?;
    assert!(
        feeds
            .iter()
            .any(|feed| feed.id == local && feed.server.is_none())
    );
    assert!(feeds.iter().any(|feed| feed.id == fixture.feed.id
        && feed.server.as_ref().is_some_and(|server| server.present)));
    Ok(())
}

#[test]
fn removing_and_reconnecting_a_server_preserves_remote_notes() -> Result<()> {
    let mut fixture = mirrored_store()?;
    let state = RemoteArticleState {
        unread: false,
        starred: true,
    };
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &[article("miniflux:1", "Cached article")],
        &update(fixture.server_id, state, 100),
    )?;
    let saved = fixture.store.library()?.articles.remove(0);
    let note = fixture.store.save_comment(saved.id, None, "Private note")?;
    fixture.store.remove_server(fixture.server_id)?;
    assert!(fixture.store.servers()?.is_empty());
    assert_eq!(fixture.store.view_count(&crate::model::View::All)?, 1);
    assert_eq!(fixture.store.comments(saved.id)?[0].id, note);
    assert_eq!(
        fixture
            .store
            .add_server("https://reader.example", "NEW_TOKEN")?,
        fixture.server_id
    );
    let article = fixture.store.article(saved.id)?;
    assert!(!article.unread);
    assert!(article.starred);
    assert_eq!(fixture.store.comments(saved.id)?[0].body, "Private note");
    Ok(())
}

#[test]
fn removing_a_server_purges_unsaved_articles_and_keeps_favorites_and_notes()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let entries =
        fixture.ingest_distinct(&["Disposable", "Favorite", "Annotated"])?;
    let remote = update(
        fixture.server_id,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        100,
    );
    let disposable = fixture.article_id("Disposable")?;
    let favorite = fixture.article_id("Favorite")?;
    let annotated = fixture.article_id("Annotated")?;
    fixture.store.set_starred(favorite, true)?;
    fixture.store.save_comment(annotated, None, "My note")?;
    fixture
        .store
        .save_smart_list("Everything", &Default::default())?;
    let list = fixture.store.smart_lists()?.remove(0).id;
    fixture.store.remove_server(fixture.server_id)?;
    assert!(fixture.store.article(disposable).is_err());
    assert!(fixture.store.article(favorite)?.starred);
    assert_eq!(fixture.store.comments(annotated)?.len(), 1);
    assert_eq!(
        fixture.store.view_count(&crate::model::View::Smart(list))?,
        2
    );
    fixture
        .store
        .add_server("https://reader.example", "TOKEN")?;
    fixture
        .store
        .ingest_remote(&fixture.feed, "Journal", &entries, &remote)?;
    assert_eq!(fixture.store.view_count(&crate::model::View::All)?, 2);
    Ok(())
}

#[test]
fn failed_source_removal_rolls_back_article_cleanup_and_source_scopes()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    fixture.store.ingest_remote(
        &fixture.feed,
        "Journal",
        &[article("miniflux:1", "Disposable")],
        &update(
            fixture.server_id,
            RemoteArticleState {
                unread: true,
                starred: false,
            },
            100,
        ),
    )?;
    fixture.store.save_smart_list(
        "Scoped",
        &crate::model::SmartFilter {
            feed: Some(fixture.feed.id),
            ..Default::default()
        },
    )?;
    fixture
        .store
        .connection
        .execute_batch(include_str!("../sql/deletion/test_reject_purge.sql"))?;
    assert!(fixture.store.remove_server(fixture.server_id).is_err());
    assert!(fixture.store.servers()?[0].enabled);
    assert_eq!(fixture.store.library()?.articles.len(), 1);
    assert_eq!(
        fixture.store.smart_lists()?[0].filter.feed,
        Some(fixture.feed.id)
    );
    Ok(())
}

#[test]
fn partial_sync_cursors_survive_storage_and_complete_without_losing_watermark()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let state = RemoteArticleState {
        unread: true,
        starred: false,
    };
    let mut partial = update(fixture.server_id, state, 100);
    partial.sync_after_id = 2000;
    partial.sync_latest = 500;
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &[],
        &partial,
    )?;
    let saved = fixture
        .store
        .feeds()?
        .remove(0)
        .server
        .ok_or_else(|| anyhow::anyhow!("Remote cursor"))?;
    assert_eq!(saved.sync_after_id, 2000);
    assert_eq!(saved.last_changed, 100);
    assert_eq!(saved.sync_latest, 500);
    let complete = update(fixture.server_id, state, 500);
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote journal",
        &[],
        &complete,
    )?;
    let saved = fixture
        .store
        .feeds()?
        .remove(0)
        .server
        .ok_or_else(|| anyhow::anyhow!("Completed cursor"))?;
    assert_eq!(saved.sync_after_id, 0);
    assert_eq!(saved.last_changed, 500);
    Ok(())
}

#[test]
fn resuming_or_reconnecting_a_server_clears_stale_feed_failures() -> Result<()>
{
    let fixture = mirrored_store()?;
    let store = &fixture.store;
    for reconnect in [false, true] {
        store.record_source_attempt(fixture.feed.id, 100, "", 0)?;
        store.record_source_attempt(fixture.feed.id, 101, "HTTP 401", 900)?;
        store.record_server_attempt(fixture.server_id, 101, "HTTP 401")?;
        if reconnect {
            store.add_server("https://reader.example", "NEW_TOKEN")?;
        } else {
            store.set_server_enabled(fixture.server_id, false)?;
            store.set_server_enabled(fixture.server_id, true)?;
        }
        let health = store.source_health()?.remove(0);
        assert_eq!(health.retry_at, 0);
        assert_eq!(health.failures, 0);
        assert!(health.error.is_empty());
        assert_eq!(health.succeeded_at, 100);
        assert!(store.servers()?[0].error.is_empty());
    }
    Ok(())
}

#[test]
fn retention_tombstones_keep_remote_aliases_removed_after_publisher_changes()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let remote = update(
        fixture.server_id,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        100,
    );
    let mut entries = [
        article("miniflux:1", "First"),
        article("miniflux:2", "Alias"),
    ];
    fixture
        .store
        .ingest_remote(&fixture.feed, "Journal", &entries, &remote)?;
    let id = fixture.store.library()?.articles[0].id;
    fixture.expire_unread(id)?;
    let images = fixture._directory.path().join("images");
    crate::retention::preview(&fixture.store, &images, 1_800_000_000)?;
    assert_eq!(
        crate::retention::apply(
            &fixture.store,
            &images,
            1_800_000_000,
            &std::sync::atomic::AtomicBool::new(false),
            None,
        )?
        .articles,
        1
    );
    entries[0].url = "https://publisher.example/moved-first".into();
    entries[1].url = "https://publisher.example/moved-alias".into();
    fixture
        .store
        .ingest_remote(&fixture.feed, "Journal", &entries, &remote)?;
    assert!(fixture.store.library()?.articles.is_empty());
    Ok(())
}

#[test]
fn removing_a_server_removes_its_retention_references_atomically() -> Result<()>
{
    let fixture = mirrored_store()?;
    let key = fixture.store.capture_configuration()?.servers[0].id.clone();
    let mut policy = crate::config::retention::Retention::default();
    policy.sources.push(crate::config::retention::Source {
        id: format!("server:{key}:7"),
        full_text_days: None,
        image_days: None,
        read_days: None,
        unread_days: None,
    });
    fixture.store.save_retention_policy(&policy)?;
    fixture.store.remove_server(fixture.server_id)?;
    assert!(fixture.store.servers()?.is_empty());
    policy.sources.clear();
    assert_eq!(fixture.store.capture_configuration()?.retention, policy);
    Ok(())
}

#[test]
fn replacing_an_inline_key_clears_stale_health_and_preserves_private_storage()
-> Result<()> {
    let fixture = mirrored_store()?;
    let path = fixture._directory.path().join("config.toml");
    crate::config::Session::open(&fixture.store, &path)?;
    for key in ["inline-first-key", "inline-rotated-key"] {
        fixture.store.record_source_attempt(
            fixture.feed.id,
            100,
            "HTTP 401",
            900,
        )?;
        fixture.store.record_server_attempt(
            fixture.server_id,
            100,
            "HTTP 401",
        )?;
        let mut config = fixture.store.capture_configuration()?;
        config.servers[0].token_env.clear();
        config.servers[0].token = Some(key.into());
        std::fs::write(&path, crate::config::encode(&config)?)?;
        crate::config::Session::open(&fixture.store, &path)?;
        let health = fixture.store.source_health()?.remove(0);
        assert!(health.error.is_empty());
        assert_eq!((health.failures, health.retry_at), (0, 0));
        let saved = fixture.store.servers()?.remove(0);
        assert!(saved.error.is_empty());
        assert_eq!(saved.token.as_ref().map(|token| token.expose()), Some(key));
        assert!(!format!("{saved:?}").contains(key));
    }
    Ok(())
}

#[test]
fn history_checkpoint_survives_restart_and_reset_retains_personal_data()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let mut remote = update(
        fixture.server_id,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        100,
    );
    remote.initialized = false;
    remote.history = crate::model::ServerHistory {
        before_id: 40,
        loaded: 2,
        total: 5,
    };
    fixture.store.ingest_remote(
        &fixture.feed,
        "Remote",
        &[article("miniflux:1", "Kept")],
        &remote,
    )?;
    let id = fixture.store.library()?.articles[0].id;
    fixture.store.set_starred(id, true)?;
    let reopened =
        Store::open(&fixture._directory.path().join("library.sqlite3"))?;
    let history = remote_scope(&reopened)?;
    assert!(!history.initialized);
    assert_eq!(history.history, remote.history);
    assert!(history.sync_progress().contains("2 / 5"));
    reopened.restart_server_history(fixture.server_id)?;
    let reset = remote_scope(&reopened)?;
    assert_eq!(reset.history.loaded, 0);
    assert_eq!(reset.history.before_id, 0);
    assert!(reopened.article(id)?.starred);
    assert!(!reopened.body(id)?.is_empty());
    assert!(reopened.restart_server_history(999).is_err());
    Ok(())
}

fn remote_scope(store: &Store) -> Result<crate::model::ServerFeed> {
    store
        .feeds()?
        .into_iter()
        .find_map(|feed| feed.server)
        .ok_or_else(|| anyhow::anyhow!("Missing remote scope"))
}

#[path = "collection_tests.rs"]
mod collection;
