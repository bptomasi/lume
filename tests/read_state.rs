//! Read is a live destination over article state, never copied membership.

mod common;

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    model::{SmartFilter, View},
    store::Store,
};

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    for character in format!(":{text}").chars() {
        press(app, KeyCode::Char(character))?;
    }
    press(app, KeyCode::Enter)
}

#[test]
fn marking_read_populates_read_without_losing_personal_data() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    let folder = store.create_collection("Research")?;
    store.add_member(folder, id)?;
    store.set_starred(id, true)?;
    store.save_comment(id, None, "Keep this insight")?;
    let mut app = App::new(store, false)?;

    press(&mut app, KeyCode::Char('m'))?;
    command(&mut app, "read")?;

    assert_eq!(app.view, View::Read);
    assert_eq!(app.page_total, 1);
    assert_eq!(app.current().map(|article| article.id), Some(id));
    let stored = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(stored.view_count(&View::Inbox)?, 0);
    assert!(stored.article(id)?.starred);
    assert_eq!(stored.comments(id)?[0].body, "Keep this insight");
    assert!(stored.article_memberships(id)?.contains(&folder));
    Ok(())
}

#[test]
fn returning_to_unread_removes_from_read_and_undo_restores_it() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    let mut app = App::new(store, false)?;
    command(&mut app, "mark-read")?;
    command(&mut app, "read")?;

    command(&mut app, "mark-unread")?;

    assert_eq!(app.page_total, 0);
    assert!(app.current().is_none());
    let stored = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(stored.view_count(&View::Read)?, 0);
    assert_eq!(stored.view_count(&View::Inbox)?, 1);
    command(&mut app, "undo")?;
    assert_eq!(app.page_total, 1);
    assert_eq!(app.current().map(|article| article.id), Some(id));
    Ok(())
}

#[test]
fn explicit_mark_read_is_idempotent_and_one_undo_reverts_it() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.set_view(View::All)?;

    command(&mut app, "mark-read")?;
    command(&mut app, "mark-read")?;
    command(&mut app, "undo")?;

    assert!(app.current().is_some_and(|article| article.unread));
    command(&mut app, "read")?;
    assert_eq!(app.page_total, 0);
    assert!(command(&mut app, "mark-read").is_err());
    Ok(())
}

#[test]
fn opening_an_article_updates_read_and_restores_that_folder_after_restart()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let id = app.current().ok_or_else(|| anyhow!("Missing article"))?.id;
    press(&mut app, KeyCode::Enter)?;
    let read_count = app
        .navigation
        .iter()
        .find(|item| item.view == View::Read)
        .map(|item| item.count);
    assert_eq!(read_count, Some(1));
    command(&mut app, "read")?;
    app.save_session()?;
    drop(app);

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let restored = App::new(store, false)?;

    assert_eq!(restored.view, View::Read);
    assert_eq!(restored.current().map(|article| article.id), Some(id));
    assert_eq!(restored.page_total, 1);
    Ok(())
}

#[test]
fn opening_updates_boolean_read_sensitive_counts_and_is_idempotent()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    for (name, text) in [
        ("Pending", "(is:unread OR title:absent) AND NOT is:read"),
        ("Done", "NOT is:unread"),
        ("Everything", "NOT title:absent"),
    ] {
        store.save_smart_list(
            name,
            &SmartFilter {
                text: text.into(),
                ..SmartFilter::default()
            },
        )?;
    }
    let mut app = App::new(store, false)?;
    app.set_view(View::All)?;

    press(&mut app, KeyCode::Enter)?;
    press(&mut app, KeyCode::Enter)?;

    for (name, expected) in [("Pending", 0), ("Done", 1), ("Everything", 1)] {
        let item = app.navigation.iter().find(|item| item.title == name);
        assert_eq!(item.map(|item| item.count), Some(expected), "{name}");
    }
    command(&mut app, "undo")?;
    assert!(app.current().is_some_and(|article| article.unread));
    Ok(())
}
