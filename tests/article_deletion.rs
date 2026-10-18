//! Article deletion commands and palette choices retain explicit user intent.

mod common;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use lume::{
    app::{App, Focus, command_suggestions},
    model::{IncomingArticle, View},
    store::Store,
};

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn keys(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    keys(app, &format!(":{text}"))?;
    press(app, KeyCode::Enter)
}

fn two_articles() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    store.ingest(
        &feed,
        &feed.title,
        &[IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "second".into(),
            title: "Second article".into(),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000,
            body: "Another independent fixture.".into(),
        }],
    )?;
    let app = App::new(store, false)?;
    Ok((directory, app))
}

#[test]
fn discoverable_commands_delete_restore_and_undo_without_losing_articles()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    for alias in ["delete", "delete-post", "delete-article"] {
        assert!(
            command_suggestions(alias)
                .iter()
                .any(|c| c.name == "delete-article")
        );
    }
    command(&mut app, "delete-post")?;
    assert_eq!(app.page_total, 0);
    assert!(app.current().is_none());
    assert_eq!(
        app.navigation
            .iter()
            .find(|n| n.view == View::Trash)
            .map(|n| n.count),
        Some(1)
    );
    command(&mut app, "trash")?;
    assert_eq!(app.view, View::Trash);
    assert_eq!(app.page_total, 1);
    command(&mut app, "restore")?;
    assert_eq!(app.page_total, 0);
    press(&mut app, KeyCode::Char('u'))?;
    assert_eq!(app.page_total, 1);
    command(&mut app, "restore-article")?;
    app.set_view(View::All)?;
    assert_eq!(app.page_total, 1);
    Ok(())
}

#[test]
fn delete_shortcut_ignores_autorepeat_library_focus_and_visual_selections()
-> Result<()> {
    let (directory, mut app) = two_articles()?;
    app.focus = Focus::Library;
    press(&mut app, KeyCode::Char('D'))?;
    assert_eq!(app.page_total, 2);
    app.focus = Focus::Reader;
    keys(&mut app, "vD")?;
    assert_eq!(app.page_total, 2);
    press(&mut app, KeyCode::Esc)?;
    app.focus = Focus::Articles;
    let target = app.current().map(|a| a.id);

    press(&mut app, KeyCode::Delete)?;
    for _ in 0..20 {
        app.key(KeyEvent::new_with_kind(
            KeyCode::Delete,
            KeyModifiers::NONE,
            KeyEventKind::Repeat,
        ))?;
    }

    assert_eq!(app.page_total, 1);
    assert_ne!(app.current().map(|a| a.id), target);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.view_count(&View::Trash)?, 1);
    Ok(())
}

#[test]
fn palette_deletion_keeps_its_captured_target_after_selection_changes()
-> Result<()> {
    let (directory, mut app) = two_articles()?;
    let captured = app.library.articles[0].id;
    let other = app.library.articles[1].id;
    app.key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::CONTROL))?;
    keys(&mut app, "Delete article")?;
    app.selected = 1;
    app.reload()?;

    press(&mut app, KeyCode::Enter)?;

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(store.is_deleted(captured)?);
    assert!(!store.is_deleted(other)?);
    assert_eq!(app.current().map(|a| a.id), Some(other));
    Ok(())
}

#[test]
fn stale_saved_targets_do_not_reopen_deleted_articles_or_break_startup()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let article = app.library.articles[0].id;
    app.save_session()?;
    drop(app);
    let path = directory.path().join("library.sqlite3");
    let store = Store::open(&path)?;
    store.set_deleted(article, true)?;
    let mut app = App::new(store, false)?;
    assert!(app.current().is_none());
    app.set_view(View::Trash)?;
    app.save_session()?;
    drop(app);
    let store = Store::open(&path)?;
    store.set_deleted(article, false)?;

    let app = App::new(store, false)?;

    assert_eq!(app.view, View::Trash);
    assert!(app.current().is_none());
    Ok(())
}
