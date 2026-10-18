//! User workflows through the same input state machine as the real terminal.

mod common;

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::app::{Action, App, Focus, Mode};

#[test]
fn normal_mode_opens_reader_and_insert_mode_keeps_vim_letters() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let id = app.current().map(|article| article.id);
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.focus, Focus::Reader);
    assert!(!app.current().is_some_and(|article| article.unread));
    type_text(&mut app, ":note")?;
    press(&mut app, KeyCode::Enter)?;
    type_text(&mut app, "jk:q are my thoughts")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.comments[0].body, "jk:q are my thoughts");
    assert_eq!(app.current().map(|article| article.id), id);
    Ok(())
}

#[test]
fn invalid_comment_retains_draft_and_can_be_corrected() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, "n")?;
    assert!(
        app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))
            .is_err()
    );
    assert!(matches!(app.mode, Mode::Input(_)));
    type_text(&mut app, "ação 🦀")?;
    press(&mut app, KeyCode::Backspace)?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert_eq!(app.comments[0].body, "ação");
    Ok(())
}

#[test]
fn live_search_cancel_restores_previous_filter() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, "/nonexistent")?;
    assert!(app.visible.is_empty());
    press(&mut app, KeyCode::Esc)?;
    assert!(app.search.is_empty());
    assert_eq!(app.visible.len(), 1);
    type_text(&mut app, "/SYSTEMS")?;
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.visible.len(), 1);
    assert_eq!(app.search, "SYSTEMS");
    Ok(())
}

#[test]
fn pasted_comments_do_not_execute_commands_and_respect_utf8_limits()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, "n")?;
    app.paste(":q\nA private thought\u{1b}")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert_eq!(app.comments[0].body, ":q\nA private thought");
    type_text(&mut app, "n")?;
    app.paste(&"🦀".repeat(5000))?;
    if let Mode::Input(input) = &app.mode {
        assert_eq!(input.text.len(), 16_384);
    }
    Ok(())
}

#[test]
fn reader_end_and_resize_keep_a_full_final_page() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store =
        lume::store::Store::open(&directory.path().join("demo.sqlite3"))?;
    lume::demo::seed(&mut store)?;
    let mut app = App::new(store, true)?;
    app.resize_reader(45, 12);
    press(&mut app, KeyCode::Enter)?;
    type_text(&mut app, "G")?;
    assert_eq!(app.scroll, app.reader_lines.len() - 12);
    let previous = app.scroll;
    app.reload()?;
    assert_eq!(app.scroll, previous);
    app.resize_reader(45, 20);
    assert_eq!(app.scroll, app.reader_lines.len() - 20);
    Ok(())
}

#[test]
fn article_can_be_saved_in_multiple_lists_without_becoming_favorite()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, "aSystems")?;
    press(&mut app, KeyCode::Enter)?;
    type_text(&mut app, "aWeekend")?;
    press(&mut app, KeyCode::Enter)?;
    ensure!(
        app.library.collections.len() == 2,
        "Both lists should exist"
    );
    let stored =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    let article = app.current().map_or(0, |article| article.id);
    assert_eq!(stored.article_memberships(article)?.len(), 2);
    assert!(!app.current().is_some_and(|article| article.starred));
    type_text(&mut app, "f")?;
    assert!(app.current().is_some_and(|article| article.starred));
    Ok(())
}

#[test]
fn comment_target_survives_a_background_reload_and_no_matching_view()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let id = app.current().map_or(0, |article| article.id);
    type_text(&mut app, "nCaptured target")?;
    let other =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    other.set_unread(id, false)?;
    app.reload()?;
    assert!(app.current().is_none());
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert_eq!(other.comments(id)?[0].body, "Captured target");
    Ok(())
}

#[test]
fn empty_views_accept_navigation_and_quit_without_panicking() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store =
        lume::store::Store::open(&directory.path().join("empty.sqlite3"))?;
    let mut app = App::new(store, false)?;
    type_text(&mut app, "jjkkggGhlfn")?;
    assert!(app.current().is_none());
    assert_eq!(press(&mut app, KeyCode::Char('q'))?, Action::Quit);
    Ok(())
}

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_text(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}
