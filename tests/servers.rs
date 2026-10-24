//! Command, navigation and publication contracts for optional connections.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus},
    config,
    model::{RemoteFeedCandidate, View},
};
use ratatui::{Terminal, backend::TestBackend};

fn command(app: &mut App, text: &str) -> Result<Action> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

#[test]
fn connections_have_their_own_section_and_removal_keeps_cached_content()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let original = store.library()?.articles[0].id;
    let mut app = App::new(store, false)?;
    assert_eq!(
        command(&mut app, "server https://reader.example LUME_TOKEN")?,
        Action::Refresh
    );
    let id = app.library.servers[0].id;
    assert_eq!(app.view, View::Server(id));
    let root = app
        .navigation
        .iter()
        .find(|item| item.view == View::Server(id))
        .context("Server navigation")?;
    assert_eq!(root.group, "SERVERS");
    assert_eq!(root.count, 0);
    assert!(app.navigation.iter().any(|item| item.group == "FEEDS"));
    command(&mut app, "server-disable")?;
    assert!(!app.library.servers[0].enabled);
    assert!(
        app.navigation
            .iter()
            .any(|item| item.title.contains("paused"))
    );
    assert_eq!(command(&mut app, "server-enable")?, Action::Refresh);
    app.focus = Focus::Library;
    app.key(KeyCode::Char('d').into())?;
    assert_eq!(app.library.servers.len(), 1);
    app.key(KeyCode::Char('d').into())?;
    assert!(app.library.servers.is_empty());
    assert_eq!(app.view, View::All);
    assert_eq!(app.library.articles[0].id, original);
    Ok(())
}

#[test]
fn remote_feeds_are_nested_and_collapsed_counts_do_not_double() -> Result<()> {
    let (_directory, store, id) = remote_library()?;
    let mut app = App::new(store, false)?;
    let feed = app
        .library
        .feeds
        .iter()
        .find(|feed| feed.server.is_some())
        .context("Remote subscription")?
        .id;
    let item = app
        .navigation
        .iter()
        .find(|item| item.view == View::Feed(feed))
        .context("Remote navigation")?;
    assert_eq!(item.group, "SERVERS");
    assert!(item.title.starts_with("  · "));
    app.set_view(View::Server(id))?;
    command(&mut app, "collapse servers")?;
    assert_eq!(app.view, View::Group("SERVERS".into()));
    assert_eq!(app.navigation[app.nav_selected].count, 1);
    app.focus = Focus::Library;
    app.key(KeyCode::Enter.into())?;
    assert!(
        app.navigation
            .iter()
            .any(|item| item.view == View::Server(id))
    );
    assert!(!app.workspace.collapsed_groups.contains(&"SERVERS".into()));
    Ok(())
}

#[test]
fn server_edits_publish_and_reconnect_the_same_portable_definition()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("config.toml");
    let session = config::Session::open(&store, &path)?;
    let id = store.add_server("https://reader.example/", "LUME_TOKEN")?;
    session.flush()?;
    let config = config::decode(&std::fs::read_to_string(&path)?)?;
    let key = config.servers[0].id.clone();
    store.set_server_enabled(id, false)?;
    session.flush()?;
    assert!(
        !config::decode(&std::fs::read_to_string(&path)?)?.servers[0].enabled
    );
    store.remove_server(id)?;
    session.flush()?;
    assert!(
        config::decode(&std::fs::read_to_string(&path)?)?
            .servers
            .is_empty()
    );
    assert_eq!(
        store.add_server("https://reader.example", "OTHER_TOKEN")?,
        id
    );
    session.flush()?;
    let config = config::decode(&std::fs::read_to_string(&path)?)?;
    assert_eq!(config.servers[0].id, key);
    assert_eq!(config.servers[0].token_env, "OTHER_TOKEN");
    Ok(())
}

#[test]
fn bad_server_definitions_do_not_partially_change_the_library() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    for (url, token) in [
        ("https://reader.example/#fragment", "TOKEN"),
        ("https://reader.example/?query", "TOKEN"),
        ("https://user:secret@reader.example", "TOKEN"),
        ("file:///private", "TOKEN"),
        ("https://reader.example", "literal-token-value"),
        ("https://reader.example", "1TOKEN"),
    ] {
        assert!(store.add_server(url, token).is_err(), "{url} {token}");
    }
    assert!(store.servers()?.is_empty());
    Ok(())
}

fn cache_remote_article(
    store: &mut lume::store::Store,
    server: i64,
) -> Result<()> {
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.server.is_some())
        .context("Remote feed")?;
    let article = lume::model::IncomingArticle {
        external_id: "miniflux:1".into(),
        title: "Cached remote article".into(),
        url: "https://publisher.example/article".into(),
        author: String::new(),
        published: 1_800_000_000,
        body: "Remote content".into(),
        body_origin: lume::model::FeedOrigin::Feed,
    };
    store.ingest_remote(
        &feed,
        &feed.title,
        &[article],
        &lume::model::RemoteFeedUpdate {
            history: Default::default(),
            server_id: server,
            remote_id: 7,
            initialized: true,
            last_changed: 100,
            sync_after_id: 0,
            sync_latest: 100,
            states: Default::default(),
        },
    )
}

#[test]
fn connection_limits_count_visible_definitions_and_allow_reconnection()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let first = store.add_server("https://reader0.example", "LUME_TOKEN")?;
    for index in 1..16 {
        store.add_server(
            &format!("https://reader{index}.example"),
            "LUME_TOKEN",
        )?;
    }
    assert!(
        store
            .add_server("https://overflow.example", "TOKEN")
            .is_err()
    );
    assert_eq!(
        store.add_server("https://reader0.example", "ROTATED_TOKEN")?,
        first
    );
    store.remove_server(first)?;
    store.add_server("https://replacement.example", "TOKEN")?;
    assert_eq!(store.servers()?.len(), 16);
    Ok(())
}

fn remote_library() -> Result<(tempfile::TempDir, lume::store::Store, i64)> {
    let (directory, mut store, _) = common::seeded_store()?;
    let id = store.add_server("https://reader.example", "LUME_TOKEN")?;
    store.sync_server_feeds(
        id,
        &[RemoteFeedCandidate {
            remote_id: 7,
            title: "Remote journal".into(),
            remote_url: "https://publisher.example/feed".into(),
            local_url: "https://reader.example/v1/feeds/7".into(),
        }],
    )?;
    cache_remote_article(&mut store, id)?;
    Ok((directory, store, id))
}

#[test]
fn paused_servers_remain_identifiable_in_a_narrow_library() -> Result<()> {
    let (_directory, store, id) = remote_library()?;
    store.set_server_enabled(id, false)?;
    let app = App::new(store, false)?;
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
    terminal.draw(|frame| lume::ui::draw(frame, &app))?;
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("SERVERS"));
    assert!(text.contains("[paused]"));
    Ok(())
}

#[test]
fn pinning_a_server_keeps_collapsed_child_counts_visible() -> Result<()> {
    let (_directory, store, id) = remote_library()?;
    let mut app = App::new(store, false)?;
    app.set_view(View::Server(id))?;
    command(&mut app, "pin")?;
    command(&mut app, "collapse servers")?;
    assert_eq!(app.navigation[app.nav_selected].count, 1);
    assert!(app.navigation.iter().any(|item| item.view == View::Server(id)
        && item.group == "PINNED"));
    Ok(())
}

#[cfg(unix)]
#[test]
fn invalid_unicode_credentials_are_never_persisted_in_errors() -> Result<()> {
    use std::os::unix::ffi::OsStringExt;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("config.toml");
    std::fs::write(
        &path,
        "[[servers]]\nid = \"home\"\nurl = \"https://reader.invalid\"\ntoken_env = \"LUME_BAD_TOKEN\"\n",
    )?;
    let result = std::process::Command::new(env!("CARGO_BIN_EXE_lume"))
        .arg("--config")
        .arg(&path)
        .arg("--data-dir")
        .arg(directory.path())
        .arg("sync")
        .env(
            "LUME_BAD_TOKEN",
            std::ffi::OsString::from_vec(b"SECRET_TOKEN\xff".to_vec()),
        )
        .output()?;
    assert!(!result.status.success());
    let store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    let servers = store.servers()?;
    assert_eq!(servers.len(), 1);
    assert!(servers[0].error.contains("LUME_BAD_TOKEN"));
    assert!(!servers[0].error.contains("SECRET_TOKEN"));
    Ok(())
}

#[test]
fn the_connection_picker_reveals_servers_in_folded_sections() -> Result<()> {
    let (_directory, store, id) = remote_library()?;
    let mut app = App::new(store, false)?;
    app.set_view(View::Server(id))?;
    command(&mut app, "collapse servers")?;
    command(&mut app, "servers")?;
    app.key(KeyCode::Enter.into())?;
    assert_eq!(app.view, View::Server(id));
    command(&mut app, "pin")?;
    command(&mut app, "collapse pinned")?;
    command(&mut app, "servers")?;
    app.key(KeyCode::Enter.into())?;
    assert_eq!(app.view, View::Server(id));
    Ok(())
}

fn remote_scope_configuration() -> Result<String> {
    let (directory, store, _) = remote_library()?;
    let path = directory.path().join("config.toml");
    let session = config::Session::open(&store, &path)?;
    let feed = remote_feed_id(&store)?;
    store.save_smart_list(
        "Remote scope",
        &lume::model::SmartFilter {
            feed: Some(feed),
            feeds: vec![feed],
            ..Default::default()
        },
    )?;
    session.flush()?;
    Ok(std::fs::read_to_string(&path)?)
}

#[test]
fn remote_smart_scopes_publish_without_synthetic_subscriptions() -> Result<()> {
    let text = remote_scope_configuration()?;
    let config = config::decode(&text)?;
    let reference = format!("server:{}:7", config.servers[0].id);
    assert_eq!(config.smart_lists[0].feed.as_ref(), Some(&reference));
    assert_eq!(config.smart_lists[0].feeds, vec![reference]);
    assert!(
        !config
            .feeds
            .iter()
            .any(|feed| feed.url.contains("/v1/feeds"))
    );
    Ok(())
}

#[test]
fn restored_remote_scopes_wait_for_discovery_and_retain_identity() -> Result<()>
{
    let text = remote_scope_configuration()?;
    let restored = tempfile::tempdir()?;
    let restored_path = restored.path().join("config.toml");
    std::fs::write(&restored_path, text)?;
    let mut restored_store =
        lume::store::Store::open(&restored.path().join("library.sqlite3"))?;
    let restored_session =
        config::Session::open(&restored_store, &restored_path)?;
    let list = restored_store.smart_lists()?.remove(0);
    assert_eq!(list.filter.feed, list.filter.feeds.first().copied());
    assert!(
        restored_store.feeds()?.iter().all(|feed| feed
            .server
            .as_ref()
            .is_none_or(|remote| !remote.present))
    );
    verify_restored_scope(
        &mut restored_store,
        list.filter.feed.context("Scope")?,
    )?;
    restored_session.flush()?;
    Ok(())
}

fn verify_restored_scope(
    store: &mut lume::store::Store,
    feed: i64,
) -> Result<()> {
    let id = store.servers()?[0].id;
    store.sync_server_feeds(
        id,
        &[RemoteFeedCandidate {
            remote_id: 7,
            title: "Discovered".into(),
            remote_url: "https://publisher.example/feed".into(),
            local_url: "https://reader.example/v1/feeds/7".into(),
        }],
    )?;
    let discovered = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.server.is_some())
        .context("Discovered remote feed")?;
    assert_eq!(discovered.id, feed);
    cache_remote_article(store, id)?;
    let list = store.smart_lists()?.remove(0);
    assert_eq!(store.view_count(&View::Smart(list.id))?, 1);
    Ok(())
}

#[test]
fn removing_a_referenced_server_keeps_its_list_empty_and_configuration_valid()
-> Result<()> {
    let (directory, store, id) = remote_library()?;
    let path = directory.path().join("config.toml");
    let session = config::Session::open(&store, &path)?;
    let feed = remote_feed_id(&store)?;
    store.save_smart_list(
        "Remote scope",
        &lume::model::SmartFilter {
            feeds: vec![feed],
            text: "body:rust".into(),
            ..Default::default()
        },
    )?;
    session.flush()?;
    store.remove_server(id)?;
    session.flush()?;
    assert!(store.servers()?.is_empty());
    let configuration = config::read_document(&path)?;
    assert!(configuration.servers.is_empty());
    assert_eq!(configuration.smart_lists.len(), 1);
    let list = store.smart_lists()?.remove(0);
    assert_eq!(list.name, "Remote scope");
    assert!(list.filter.feeds.is_empty());
    assert!(list.filter.text.contains("body:rust"));
    assert_eq!(store.view_count(&View::Smart(list.id))?, 0);
    assert!(
        store
            .save_smart_list(
                "Invalid",
                &lume::model::SmartFilter {
                    feed: Some(feed),
                    ..Default::default()
                }
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn removing_one_server_keeps_other_smart_list_sources() -> Result<()> {
    let (directory, store, id) = remote_library()?;
    let remote = remote_feed_id(&store)?;
    let local = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.server.is_none())
        .context("Local source")?
        .id;
    store.save_smart_list(
        "Mixed",
        &lume::model::SmartFilter {
            feeds: vec![remote, local],
            ..Default::default()
        },
    )?;
    let path = directory.path().join("config.toml");
    let session = config::Session::open(&store, &path)?;
    store.remove_server(id)?;
    session.flush()?;
    let lists = store.smart_lists()?;
    let mixed = lists
        .iter()
        .find(|list| list.name == "Mixed")
        .context("Mixed")?;
    assert_eq!(mixed.filter.feeds, vec![local]);
    assert!(mixed.filter.text.is_empty());
    config::read_document(&path)?;
    Ok(())
}

#[test]
fn removing_a_server_preserves_a_query_at_its_byte_limit() -> Result<()> {
    let (directory, store, id) = remote_library()?;
    let query = format!("body:{}", "r".repeat(4091));
    store.save_smart_list(
        "Boundary",
        &lume::model::SmartFilter {
            text: query.clone(),
            feed: Some(remote_feed_id(&store)?),
            ..Default::default()
        },
    )?;
    let path = directory.path().join("config.toml");
    let session = config::Session::open(&store, &path)?;
    store.remove_server(id)?;
    session.flush()?;
    let list = store.smart_lists()?.remove(0);
    assert_eq!(list.filter.text, query);
    assert!(list.filter.paused);
    assert_eq!(store.view_count(&View::Smart(list.id))?, 0);
    assert!(config::read_document(&path)?.smart_lists[0].paused);
    Ok(())
}

fn remote_feed_id(store: &lume::store::Store) -> Result<i64> {
    Ok(store
        .feeds()?
        .into_iter()
        .find(|feed| feed.server.is_some())
        .context("Remote feed")?
        .id)
}

#[test]
fn malformed_remote_scope_references_are_rejected_before_application() {
    for reference in [
        "server:home",
        "server:home:0",
        "server:home:-1",
        "server:home:7:extra",
        "server:unknown:7",
        "server::7",
    ] {
        let text = format!(
            "[[servers]]\nid='home'\nurl='https://reader.example'\ntoken_env='TOKEN'\n\
             [[smart_lists]]\nid='scope'\nname='Scope'\nfeed='{reference}'\n"
        );
        assert!(config::decode(&text).is_err(), "{reference}");
    }
}

#[test]
fn masked_token_command_publishes_the_key_without_exposing_it_in_frames_or_debug()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("config.toml");
    let session = config::Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());
    assert_eq!(
        command(&mut app, "server https://reader.example --token")?,
        Action::None
    );
    assert!(app.library.servers.is_empty());
    app.paste("private-ui-token\n")?;
    assert!(!format!("{:?}", app.mode).contains("private-ui-token"));
    let mut terminal = Terminal::new(TestBackend::new(100, 32))?;
    terminal.draw(|frame| lume::ui::draw(frame, &app))?;
    let pixels = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(pixels.contains('•'));
    assert!(pixels.contains("reader.example"));
    assert!(!pixels.contains("private-ui-token"));
    assert_eq!(app.key(KeyCode::Enter.into())?, Action::Refresh);
    session.flush()?;
    let saved = config::read_document(&path)?;
    assert_eq!(
        saved.servers[0].token.as_ref().map(|key| key.expose()),
        Some("private-ui-token")
    );
    assert!(saved.servers[0].token_env.is_empty());
    assert!(!format!("{:?}", app.library.servers).contains("private-ui-token"));
    Ok(())
}

#[test]
fn token_entry_cancels_and_rejects_controls_or_excessive_pastes_without_partial_connections()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "server https://reader.example --token")?;
    assert!(app.key(KeyCode::Enter.into()).is_err());
    assert!(app.paste("bad\nkey").is_err());
    assert!(app.paste(&"x".repeat(4097)).is_err());
    app.paste("discarded-secret")?;
    app.key(KeyCode::Esc.into())?;
    assert!(app.library.servers.is_empty());
    assert!(matches!(app.mode, lume::app::Mode::Normal));
    command(&mut app, "server https://reader.example --token")?;
    app.paste("replace-this")?;
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))?;
    app.paste("final-token")?;
    app.key(KeyCode::Char('x').into())?;
    app.key(KeyCode::Backspace.into())?;
    assert_eq!(app.key(KeyCode::Enter.into())?, Action::Refresh);
    assert_eq!(
        app.library.servers[0]
            .token
            .as_ref()
            .map(|key| key.expose()),
        Some("final-token")
    );
    Ok(())
}
