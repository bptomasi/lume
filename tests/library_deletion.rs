//! Sidebar deletion captures definitions while preserving cached personal data.
mod common;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use lume::{
    app::{App, Focus},
    config::{self, Session},
    model::{SmartFilter, View},
    store::Store,
};

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn select(app: &mut App, target: View) -> Result<()> {
    app.focus = Focus::Library;
    app.nav_selected = app
        .navigation
        .iter()
        .position(|item| item.view == target)
        .context("Definition missing from Library")?;
    Ok(())
}

#[test]
fn dd_removes_the_captured_feed_and_smart_list_and_publishes_configuration()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    store.save_smart_list("Research", &SmartFilter::default())?;
    let smart = store.smart_lists()?.remove(0).id;
    let article = store.library()?.articles.remove(0);
    store.save_comment(article.id, None, "Keep this note")?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());
    for target in [View::Feed(feed.id), View::Smart(smart)] {
        select(&mut app, target.clone())?;
        key(&mut app, KeyCode::Char('d'))?;
        assert!(app.navigation.iter().any(|item| item.view == target));
        key(&mut app, KeyCode::Char('d'))?;
        assert!(!app.navigation.iter().any(|item| item.view == target));
    }
    session.flush()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(store.feeds()?.is_empty());
    assert!(store.smart_lists()?.is_empty());
    assert!(!store.body(article.id)?.is_empty());
    assert_eq!(store.comments(article.id)?.len(), 1);
    let document = config::read_document(&path)?;
    assert!(!document.feeds[0].enabled);
    assert!(document.smart_lists.is_empty());
    Ok(())
}

#[test]
fn repeats_cancellation_and_target_changes_cannot_remove_another_definition()
-> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    store.save_smart_list("Research", &SmartFilter::default())?;
    let smart = store.smart_lists()?.remove(0).id;
    let mut app = App::new(store, false)?;
    select(&mut app, View::Feed(feed.id))?;
    key(&mut app, KeyCode::Char('d'))?;
    let mut repeat = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::NONE);
    repeat.kind = KeyEventKind::Repeat;
    app.key(repeat)?;
    key(&mut app, KeyCode::Esc)?;
    assert!(
        app.navigation
            .iter()
            .any(|item| item.view == View::Feed(feed.id))
    );
    key(&mut app, KeyCode::Char('d'))?;
    select(&mut app, View::Smart(smart))?;
    key(&mut app, KeyCode::Char('d'))?;
    assert!(
        app.navigation
            .iter()
            .any(|item| item.view == View::Smart(smart))
    );
    key(&mut app, KeyCode::Esc)?;
    select(&mut app, View::Group("SMART LISTS".into()))?;
    for _ in 0..2 {
        key(&mut app, KeyCode::Char('d'))?;
    }
    assert!(app.current().is_some());
    Ok(())
}

#[test]
fn background_sidebar_removal_uses_the_ordered_write_lane() -> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    select(&mut app, View::Feed(feed.id))?;
    for _ in 0..2 {
        key(&mut app, KeyCode::Char('d'))?;
    }
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        common::settle_notes(&mut app)?;
        if !app.catalog_pending() {
            break;
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Catalog timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(
        !app.navigation
            .iter()
            .any(|item| item.view == View::Feed(feed.id))
    );
    assert!(!app.status.contains("u undo"));
    app.end_session()?;
    Ok(())
}

#[test]
fn rejected_removal_preserves_pins_order_and_the_active_destination()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    rusqlite::Connection::open(&path)?.execute_batch(include_str!(
        "../src/store/sql/deletion/test_reject_purge.sql"
    ))?;
    let mut app = App::new(store, false)?;
    let view = View::Feed(feed.id);
    app.set_view(view.clone())?;
    app.workspace.pins.push(view.clone());
    app.workspace.navigation_order.push(view.clone());
    app.save_session()?;
    app.enable_background_navigation(&path)?;
    select(&mut app, view.clone())?;
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Char('d'))?;
    let error = common::settle_notes(&mut app)
        .err()
        .context("Deletion rejection")?;
    assert!(error.to_string().contains("Controlled permanent deletion"));
    assert_eq!(app.view, view);
    assert!(app.workspace.pins.contains(&view));
    assert!(app.workspace.navigation_order.contains(&view));
    let observer = Store::open(&path)?;
    assert_eq!(observer.feeds()?.len(), 1);
    assert_eq!(observer.library()?.articles.len(), 1);
    rusqlite::Connection::open(&path)?.execute_batch(include_str!(
        "../src/store/sql/deletion/test_allow_purge.sql"
    ))?;
    key(&mut app, KeyCode::Char('d'))?;
    key(&mut app, KeyCode::Char('d'))?;
    common::settle_notes(&mut app)?;
    assert!(observer.feeds()?.is_empty());
    app.end_session()
}

#[test]
fn repeated_removal_before_acknowledgement_is_admitted_only_once() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let server = store.add_server("https://reader.example", "TOKEN")?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    select(&mut app, View::Server(server))?;
    for _ in 0..4 {
        key(&mut app, KeyCode::Char('d'))?;
    }
    common::settle_notes(&mut app)?;
    assert!(Store::open(&path)?.servers()?.is_empty());
    app.end_session()
}
