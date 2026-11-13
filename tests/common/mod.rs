//! Small scenario setup shared by native integration tests.

use anyhow::{Result, anyhow};
use lume::{model::Feed, source, store::Store};

#[allow(dead_code, reason = "Each integration target selects its own setup")]
pub fn seeded_store() -> Result<(tempfile::TempDir, Store, Feed)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let id = store.subscribe("https://journal.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .ok_or_else(|| anyhow!("Missing scenario subscription"))?;
    let parsed =
        source::parse(include_bytes!("../fixtures/rss.xml"), &feed.url)?;
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    Ok((directory, store, feed))
}

/// Settle annotation workers with a deadline, exercising their real poll path.
#[allow(
    dead_code,
    reason = "Each integration target selects its own scenarios"
)]
pub fn settle_notes(app: &mut lume::app::App) -> Result<()> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.poll_reader();
        app.poll_navigation()?;
        app.poll_note_workspace()?;
        if !app.navigation().navigation_loading
            && !app.notes_pending()
            && !app.notebook_pending()
            && !app.note_inspection_pending()
            && !app.note_catalog_pending()
        {
            return Ok(());
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Note worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

pub mod reader;

/// Drive fixture preferences through the same command entry as terminal input.
#[allow(dead_code, reason = "Each target selects its fixture operations")]
pub fn fixture_command(app: &mut lume::app::App, command: &str) -> Result<()> {
    use crossterm::event::KeyCode;
    app.key(KeyCode::Char(':').into())?;
    app.paste(command)?;
    app.key(KeyCode::Enter.into())?;
    Ok(())
}

#[allow(dead_code, reason = "Each target selects its fixture operations")]
pub fn fixture_toggle(
    app: &mut lume::app::App,
    setting: &str,
    enabled: bool,
) -> Result<()> {
    let current = match setting {
        "fullscreen" => app.workspace().fullscreen,
        "zen" => app.workspace().zen,
        "compact" => app.workspace().compact,
        _ => anyhow::bail!("Unknown fixture toggle: {setting}"),
    };
    if current != enabled {
        fixture_command(app, setting)?;
    }
    Ok(())
}

#[allow(dead_code, reason = "Each target selects its fixture operations")]
pub fn fixture_pane(
    app: &mut lume::app::App,
    pane: lume::model::Pane,
    visible: bool,
) -> Result<()> {
    if app.workspace().pane_visible(pane) != visible {
        fixture_command(app, &format!("panel {}", pane.name()))?;
    }
    Ok(())
}

#[allow(dead_code, reason = "Each target selects its fixture operations")]
pub fn fixture_setting(
    app: &mut lume::app::App,
    field: usize,
    value: &str,
) -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    fixture_command(app, "settings")?;
    for _ in 0..field {
        app.key(KeyCode::Down.into())?;
    }
    app.key(KeyCode::Enter.into())?;
    app.paste(value)?;
    app.key(KeyCode::Enter.into())?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    Ok(())
}
