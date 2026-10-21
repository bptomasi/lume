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

#[test]
fn image_preferences_survive_publication_and_reopening() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = Store::open(&database)?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());
    command(&mut app, "images off")?;
    command(&mut app, "image-protocol sixel")?;
    app.end_session()?;
    session.flush()?;

    let config = config::read_document(&path)?;
    assert!(!config.ui.images.enabled);
    assert_eq!(
        config.ui.images.protocol,
        lume::model::ImagePreference::Sixel
    );
    let reopened = App::new(Store::open(&database)?, false)?;
    assert!(!reopened.images.enabled);
    assert_eq!(reopened.workspace.images, config.ui.images);
    Ok(())
}

#[test]
fn source_fetch_policy_round_trips_declaratively() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = Store::open(&database)?;
    let feed = store.subscribe("https://journal.example/feed")?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    store.set_fetch_policy(feed, lume::model::FetchPolicy::Manual)?;
    session.flush()?;
    assert_eq!(
        config::read_document(&path)?.feeds[0].fetch_policy,
        lume::model::FetchPolicy::Manual
    );
    let text = fs::read_to_string(&path)?.replace("manual", "always");
    fs::write(&path, text)?;
    Session::open(&store, &path)?;
    assert_eq!(store.fetch_policy(feed)?, lume::model::FetchPolicy::Always);
    Ok(())
}

#[test]
fn justification_alone_publishes_and_external_edits_survive_restart()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = Store::open(&database)?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());
    app.save_session()?;
    session.flush()?;

    command(&mut app, "justify on")?;
    session.flush()?;
    assert!(config::read_document(&path)?.ui.justify);
    let reopened = App::new(Store::open(&database)?, false)?;
    assert!(reopened.workspace.justify);

    command(&mut app, "justify off")?;
    session.flush()?;
    assert!(!config::read_document(&path)?.ui.justify);
    fs::write(&path, "[ui]\njustify = true\n")?;
    let external = Session::open(&Store::open(&database)?, &path)?;
    app.configuration_updated(external.initial().clone())?;
    assert!(app.workspace.justify);
    assert!(App::new(Store::open(&database)?, false)?.workspace.justify);
    Ok(())
}

#[test]
fn external_preferences_wait_for_a_polled_barrier_without_blocking_input()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = Store::open(&database)?;
    let session = Session::open(&store, &directory.path().join("config.toml"))?;
    let mut update = session.initial().clone();
    update.ui.theme = "nord".into();
    let mut app = App::new(store, false)?;
    app.configure_document(session.initial().clone());
    app.enable_background_navigation(&database)?;
    let blocker = rusqlite::Connection::open(&database)?;
    blocker.execute_batch("BEGIN IMMEDIATE")?;
    app.save_session()?;

    let started = std::time::Instant::now();
    app.configuration_updated(update)?;

    assert!(started.elapsed() < std::time::Duration::from_millis(250));
    assert_eq!(app.workspace.theme, "lume");
    blocker.execute_batch("ROLLBACK")?;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app.workspace.theme != "nord" {
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Configuration barrier timed out"
        );
        app.poll_navigation()?;
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    app.end_session()?;
    Ok(())
}
