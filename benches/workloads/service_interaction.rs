//! Selector input stays independent of SQLite and page layout runs off-thread.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode, Operation},
    model::WebDocument,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    selector(iterations, stress)?;
    web(iterations, stress)
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.picker_pending() || app.web_pending() {
        app.poll_picker()?;
        app.poll_web()?;
        ensure!(Instant::now() < deadline, "Service worker did not settle");
        std::thread::yield_now();
    }
    Ok(())
}

fn selector(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 2000 } else { 200 };
    let (directory, store) = super::reading_services::fixture(count)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
    key(&mut app, KeyCode::Char(' '))?;
    key(&mut app, KeyCode::Char('f'))?;
    settle(&mut app)?;
    ensure!(
        matches!(&app.mode, Mode::Picker(picker) if picker.choices.len() == 50),
        "Selector lost results"
    );
    super::measure(
        "selector sweep input + frame (50 choices)",
        iterations * 10,
        || {
            for direction in [KeyCode::Down, KeyCode::Up] {
                key(&mut app, direction)?;
                terminal.draw(|frame| ui::draw(frame, &app))?;
            }
            ensure!(
                matches!(&app.mode, Mode::Picker(picker) if picker.query.is_empty() && picker.selected == 0),
                "Selection drifted"
            );
            Ok(())
        },
    )?;
    settle(&mut app)?;
    ensure!(
        matches!(&app.mode, Mode::Picker(picker) if picker.preview.contains("Publisher feed remains")),
        "Preview is stale"
    );
    app.end_session()?;
    Ok(())
}

fn web(iterations: usize, stress: bool) -> Result<()> {
    let (directory, store) = super::reading_services::fixture(1)?;
    let sections = if stress { 500 } else { 50 };
    let body =
        "## Section\n\nReadable linked content with **strong** words.\n\n"
            .repeat(sections);
    let document = WebDocument {
        url: "https://example.org/linked".into(),
        title: "Linked benchmark".into(),
        body,
    };
    let id = store.save_web_document(&document)?;
    super::measure("idempotent explicit page save", iterations, || {
        ensure!(
            store.save_web_document(&document)? == id,
            "Save changed identity"
        );
        Ok(())
    })?;
    let mut app = App::new(store, false)?;
    command(&mut app, "performance on")?;
    key(&mut app, KeyCode::Esc)?;
    linked_open(&mut app, iterations, sections)?;
    drop(directory);
    Ok(())
}

fn linked_open(
    app: &mut App,
    iterations: usize,
    sections: usize,
) -> Result<()> {
    let parent = app.current().context("Parent missing")?.id;
    super::measure(
        "cached linked page admission + layout + delivery",
        iterations,
        || {
            command(app, "open-page https://example.org/linked")?;
            settle(app)?;
            let page = app.web_page().context("Linked page missing")?;
            ensure!(
                page.rendered
                    .text
                    .matches("Readable linked content")
                    .count()
                    == sections,
                "Linked content lost"
            );
            app.begin_measurement(Operation::Input);
            key(app, KeyCode::Char('l'))?;
            app.finish_measurement(Operation::Input);
            key(app, KeyCode::Esc)?;
            ensure!(
                !app.web_active()
                    && app.current().is_some_and(|item| item.id == parent),
                "Parent changed"
            );
            Ok(())
        },
    )
}
