//! Explicit durability boundaries retain every admitted personal/session edit.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{app::App, store::Store};

pub(super) fn run(iterations: usize, _stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("shutdown.sqlite3");
    let store = super::page_navigation::fixture(&path, 32)?;
    let mut app = App::new(store, false)?;
    let article = app.current().context("Missing flush article")?.id;
    app.enable_background_navigation(&path)?;
    let observer = Store::open(&path)?;
    super::measure(
        "24 session saves + 8 flags + 4 preferences + shutdown flush",
        iterations,
        || {
            // Unchanged reading positions are deduplicated by ProgressMemory.
            // These are save calls, not 24 distinct admitted progress writes.
            for _ in 0..24 {
                app.save_session()?;
            }
            for _ in 0..8 {
                app.key(KeyEvent::new(KeyCode::Char('f'), KeyModifiers::NONE))?;
            }
            for minutes in [10, 20, 30, 40] {
                command(
                    &mut app,
                    &format!("recommend-settings minutes {minutes}"),
                )?;
            }
            app.end_session()?;
            ensure!(!app.session_pending(), "Session writes remain pending");
            ensure!(!app.notes_pending(), "Personal writes remain pending");
            ensure!(
                !app.recommendations_pending(),
                "Preference writes remain pending"
            );
            ensure!(!observer.article(article)?.starred, "Flag order changed");
            ensure!(
                observer.recommendation_profile()?.minutes == Some(40),
                "Last preference was lost"
            );
            ensure!(observer.workspace()?.last_visit > 0, "Session lost");
            Ok(())
        },
    )
}

fn command(app: &mut App, command: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(command)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}
