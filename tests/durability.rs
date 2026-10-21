//! Compose failures, captured edits and shutdown without losing recovery data.

mod common;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    model::View,
    store::Store,
};

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

#[test]
fn failed_recommendation_edit_cannot_suppress_dirty_note_recovery() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    command(&mut app, "interest-remove unknown-interest")?;
    app.focus = Focus::Reader;
    command(&mut app, "note")?;
    app.paste("Recover this even if another worker fails")?;

    assert!(app.end_session().is_err());

    let store = Store::open(&path)?;
    assert_eq!(store.note_draft_count()?, 1);
    assert!(store.workspace()?.last_visit > 0);
    Ok(())
}

#[test]
fn invalid_preferences_are_rejected_before_background_admission() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    for text in [
        "recommend-settings minutes 999",
        "recommend-settings source 1 100",
        "interest Broken | 0 | rust",
    ] {
        assert!(command(&mut app, text).is_err());
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    }
    app.end_session()?;
    Ok(())
}

#[test]
fn rapid_favorite_and_queue_toggles_preserve_intentions_and_undo_order()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.view = View::All;
    let id = app
        .current()
        .map(|article| article.id)
        .ok_or_else(|| anyhow::anyhow!("Missing article"))?;
    app.enable_background_navigation(&path)?;
    app.focus = Focus::Articles;
    for character in "ffss".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    assert!(
        !app.current()
            .ok_or_else(|| anyhow::anyhow!("Missing article"))?
            .starred
    );
    app.end_session()?;

    let store = Store::open(&path)?;
    assert!(!store.article(id)?.starred);
    assert!(!store.article(id)?.queued);
    assert!(store.undo()?);
    assert!(store.article(id)?.queued);
    assert!(store.undo()?);
    assert!(!store.article(id)?.queued);
    assert!(store.undo()?);
    assert!(store.article(id)?.starred);
    Ok(())
}

#[test]
fn folder_creation_memberships_flags_and_undo_share_one_history_lane()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    command(&mut app, "folder Weekend")?;
    app.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))?;
    app.end_session()?;
    let store = Store::open(&path)?;
    let article = store.library()?.articles[0].clone();
    assert!(!article.starred);
    assert!(
        store
            .catalog()?
            .collections
            .iter()
            .any(|folder| folder.name == "Weekend")
    );
    assert!(store.undo()?);
    assert!(store.catalog()?.collections.is_empty());
    Ok(())
}
