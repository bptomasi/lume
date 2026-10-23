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
        sync_offset: 0,
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
fn partial_sync_cursors_survive_storage_and_complete_without_losing_watermark()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let state = RemoteArticleState {
        unread: true,
        starred: false,
    };
    let mut partial = update(fixture.server_id, state, 100);
    partial.sync_offset = 2000;
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
    assert_eq!(saved.sync_offset, 2000);
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
    assert_eq!(saved.sync_offset, 0);
    assert_eq!(saved.last_changed, 500);
    Ok(())
}
