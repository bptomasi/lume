//! Large justified readers preserve copying and source-bound code viewports.

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus},
    source,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const PROSE: &str =
    "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima.";

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let paragraphs = if stress { 1200 } else { 200 };
    let screen = Rect::new(0, 0, 144, 44);
    let (_directory, mut app) = reader(paragraphs, screen)?;
    let mut terminal =
        Terminal::new(TestBackend::new(screen.width, screen.height))?;
    let content = ui::workspace_reader_content(screen, &app.workspace);
    let original = app.reader_text.clone();
    println!(
        "presentation input · paragraphs={paragraphs} · bytes={}",
        app.body.len()
    );
    super::measure(
        "justified selection + exact yank + cached frame",
        iterations * 20,
        || {
            for character in "gg0wv4ly".chars() {
                let action = key(&mut app, KeyCode::Char(character))?;
                if character == 'y' {
                    ensure!(
                        action == Action::Copy("bravo".into()),
                        "Yank changed source"
                    );
                }
            }
            key(&mut app, KeyCode::Esc)?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            let line = (content.x..content.right())
                .map(|x| terminal.backend().buffer()[(x, content.y)].symbol())
                .collect::<String>();
            ensure!(
                line == "Alpha  bravo  charlie delta echo foxtrot",
                "Justified frame drifted"
            );
            ensure!(app.reader_text == original, "Readable text changed");
            Ok(())
        },
    )?;
    Ok(())
}

fn reader(paragraphs: usize, screen: Rect) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store =
        Store::open(&directory.path().join("presentation.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let mut parsed = source::parse(
        include_bytes!("../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    parsed.articles[0].body = format!("{PROSE}\n\n").repeat(paragraphs);
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.workspace.fullscreen = true;
    app.workspace.fullscreen_width = 40;
    app.focus = Focus::Reader;
    app.resize_workspace(screen);
    for character in ":justify on".chars() {
        key(&mut app, KeyCode::Char(character))?;
    }
    key(&mut app, KeyCode::Enter)?;
    Ok((directory, app))
}

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}
