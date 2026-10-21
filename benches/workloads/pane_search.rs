//! Cached sidebar matching and inline frames with a large source catalog.

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let sources = if stress { 5000 } else { 200 };
    let (_directory, mut app) = catalog(sources)?;
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    app.focus = Focus::Library;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    println!("inline search input · sources={sources}");
    super::measure(
        "live sidebar query + exact destination + frame",
        iterations * 20,
        || {
            key(&mut app, KeyCode::Char('/'))?;
            for character in "source-000".chars() {
                key(&mut app, KeyCode::Char(character))?;
            }
            ensure!(app.library_query() == "source-000", "Query was not live");
            key(&mut app, KeyCode::Down)?;
            ensure!(
                app.navigation[app.nav_selected].title
                    == "https://source-0001.example/feed",
                "Matching identity drifted"
            );
            terminal.draw(|frame| ui::draw(frame, &app))?;
            key(&mut app, KeyCode::Esc)?;
            ensure!(
                app.library_query().is_empty(),
                "Cancellation retained the filter"
            );
            Ok(())
        },
    )
}

fn catalog(count: usize) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("search.sqlite3"))?;
    for index in 0..count {
        store.subscribe(&format!("https://source-{index:04}.example/feed"))?;
    }
    Ok((directory, App::new(store, false)?))
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}
