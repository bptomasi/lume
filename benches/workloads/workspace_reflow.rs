//! Source-sized reflow workloads retain selected text and include private notes.

use std::time::Instant;

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    model::{FeedOrigin, IncomingArticle, Pane, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (directory, mut app) = application(stress)?;
    app.enable_background_navigation(&directory.path().join("reflow.sqlite3"))?;
    app.set_view(View::All)?;
    super::workspaces::settle(&mut app)?;
    let selected = select_passage(&mut app)?;
    let source = app.body.clone();
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    let mut admission = Vec::new();
    let mut completion = Vec::new();
    for _ in 0..iterations {
        for width in [40, 48, 60, 80] {
            let start = Instant::now();
            app.workspace.reading_width = width;
            app.resize_workspace(Rect::new(0, 0, 144, 44));
            terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
            admission.push(start.elapsed().as_secs_f64() * 1000.0);
            super::workspaces::settle(&mut app)?;
            terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
            completion.push(start.elapsed().as_secs_f64() * 1000.0);
            ensure!(app.body == source, "Reflow changed publisher content");
            let range =
                app.selected_range().context("Reflow lost selection")?;
            ensure!(
                app.reader_text[range] == selected,
                "Reflow changed selected bytes"
            );
        }
    }
    super::workspaces::report("background reflow admission + frame", admission);
    super::workspaces::report(
        "background completed reflow + frame",
        completion,
    );
    app.end_session()?;
    Ok(())
}

fn select_passage(app: &mut App) -> Result<String> {
    app.focus = Pane::Reader;
    for character in "ggv3l".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    let range = app.selected_range().context("Missing selection")?;
    Ok(app.reader_text[range].to_owned())
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("reflow.sqlite3"))?;
    store.subscribe("https://reflow.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let repeats = if stress { 640 } else { 64 };
    let body = format!(
        "# Layout proof\n\n{}",
        include_str!("../fixtures/rich_article.md").repeat(repeats)
    );
    let notes = if stress { 200 } else { 20 };
    println!(
        "background reflow input: {} source bytes, {notes} notes",
        body.len()
    );
    store.ingest(
        &feed,
        "Reflow fixture",
        &[IncomingArticle {
            external_id: "measured-reflow".into(),
            title: "Measured technical article".into(),
            url: "https://reflow.example/article".into(),
            author: "Fixture".into(),
            published: 1_800_000_000,
            body,
            body_origin: FeedOrigin::Feed,
        }],
    )?;
    let article = store.library()?.articles.remove(0).id;
    for index in 0..notes {
        store.save_comment(article, None, &format!("Study note {index}: retain the exact selected passage while its display width changes."))?;
    }
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    Ok((directory, app))
}
