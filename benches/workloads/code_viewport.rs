//! Deep code-row motion and selected full frames, with source checks.
//! Reader setup now dispatches normal motions/selections. Timed closures
//! include that input cost; old direct-state timings are not comparable.

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::frame_composition::draw_interactive;
use lume::{
    app::{App, Focus},
    model::{FeedOrigin, IncomingArticle},
    store::Store,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (_directory, mut app, line) = application(stress)?;
    app.activate_pane(Focus::Reader);
    crate::fixture_toggle(&mut app, "fullscreen", true)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    let row = app
        .reader()
        .code_lines
        .iter()
        .position(|code| *code)
        .unwrap_or(0);
    crate::reader_input::cursor(&mut app, row, 0)?;
    app.key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE))?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::measure(
        "deep code motion + selected full frame",
        iterations * 25,
        || {
            app.key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::NONE))?;
            app.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))?;
            let anchor = 0;
            crate::reader_input::select(&mut app, anchor)?;
            terminal.draw(|frame| draw_interactive(frame, &mut app))?;
            app.clear_copied_selection();
            let buffer = terminal.backend().buffer();
            let screen = buffer
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            ensure!(screen.contains("TAIL"), "Deep code viewport lost tail");
            ensure!(
                app.reader().reader_lines[app.reader().reader_cursor] == line,
                "Code source changed"
            );
            Ok(())
        },
    )
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App, String)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("code.sqlite3"))?;
    store.subscribe("https://code.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let repeats = if stress { 65_536 } else { 4096 };
    let line = format!("{}TAIL", "x界e\u{301}\t".repeat(repeats));
    store.ingest(
        &feed,
        "Code",
        &[IncomingArticle {
            external_id: "row".into(),
            title: "Long row".into(),
            url: "https://code.example/article".into(),
            author: "Fixture".into(),
            published: 1_800_000_000,
            body: format!("```text\n{line}\n```"),
            body_origin: FeedOrigin::Feed,
        }],
    )?;
    Ok((directory, App::new(store, false)?, line))
}
