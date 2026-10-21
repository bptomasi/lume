//! Real input dispatch publishes portable definitions and guards local previews.

use std::fs;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    config::{self, Session},
    store::Store,
};

#[test]
fn interface_commands_update_the_same_declarative_document() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());

    command(&mut app, "folder Weekend")?;
    command(&mut app, "theme nord")?;
    command(&mut app, "bind ctrl-t toc")?;
    command(&mut app, "interest Astronomy | 8 | telescope OR astronomy")?;
    command(&mut app, "workspace save My study")?;
    app.end_session()?;
    session.flush()?;

    let config = config::read_document(&path)?;
    assert_eq!(config.folders[0].name, "Weekend");
    assert_eq!(config.ui.theme, "nord");
    assert_eq!(config.ui.keybindings["ctrl-t"], "toc");
    assert_eq!(config.recommendations.interests[0].name, "Astronomy");
    assert!(
        config.workspaces.contains_key("My study"),
        "{}",
        fs::read_to_string(&path)?
    );
    Ok(())
}

#[test]
fn external_preferences_apply_without_replacing_newer_interface_choices()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = Store::open(&database)?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());
    fs::write(&path, "[ui]\ntheme = 'nord'\n")?;
    let reconciled = Session::open(&Store::open(&database)?, &path)?;

    command(&mut app, "theme gruvbox")?;
    app.configuration_updated(reconciled.initial().clone())?;
    assert_eq!(app.workspace.theme, "gruvbox");
    app.end_session()?;
    session.flush()?;
    assert_eq!(config::read_document(&path)?.ui.theme, "gruvbox");
    Ok(())
}

#[test]
fn configuration_reload_waits_for_modal_reading_and_selections() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    assert!(app.configuration_idle());
    command(&mut app, "settings")?;
    assert!(!app.configuration_idle());
    key(&mut app, KeyCode::Esc)?;
    assert!(app.configuration_idle());
    Ok(())
}

#[test]
fn nested_preferences_publish_without_changing_parent_fields() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());

    command(&mut app, "bind ctrl-t toc")?;
    app.end_session()?;
    session.flush()?;

    let config = config::read_document(&path)?;
    assert_eq!(config.ui.theme, "lume");
    assert_eq!(config.ui.keybindings["ctrl-t"], "toc");

    command(&mut app, "workspace save Focus")?;
    app.end_session()?;
    session.flush()?;

    let config = config::read_document(&path)?;
    assert_eq!(config.workspaces["Focus"], config.ui.layout);
    assert_eq!(config.ui.keybindings["ctrl-t"], "toc");
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}
