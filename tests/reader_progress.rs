//! Progress follows the reading cursor even before the viewport starts moving.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{app::App, model::Pane, source, ui};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 120, 44);

fn reader(rows: usize) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    let lines = (0..rows)
        .map(|row| format!("Reading row {row}\n"))
        .collect::<String>();
    parsed.articles[0].body = format!("```text\n{lines}```");
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.workspace.fullscreen = true;
    app.focus = Pane::Reader;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn percent(app: &App) -> Result<usize> {
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    let text = (1..10)
        .map(|x| terminal.backend().buffer()[(x, SCREEN.height - 2)].symbol())
        .collect::<String>();
    Ok(text
        .split('%')
        .next()
        .context("Missing progress label")?
        .trim()
        .parse()?)
}

#[test]
fn keyboard_progress_advances_before_any_scroll_and_returns_to_zero()
-> Result<()> {
    let (_directory, mut app) = reader(80)?;
    assert_eq!(percent(&app)?, 0);
    for _ in 0..20 {
        key(&mut app, KeyCode::Char('j'))?;
    }
    assert_eq!(app.scroll, 0);
    assert!(
        percent(&app)? >= 20,
        "Reading on the first screen stayed at 0%"
    );
    key(&mut app, KeyCode::Char('G'))?;
    assert_eq!(percent(&app)?, 100);
    for _ in 0..20 {
        key(&mut app, KeyCode::Char('k'))?;
    }
    assert!(percent(&app)? < 100);
    key(&mut app, KeyCode::Char('g'))?;
    key(&mut app, KeyCode::Char('g'))?;
    assert_eq!(percent(&app)?, 0);
    Ok(())
}

#[test]
fn short_articles_advance_with_the_cursor_without_requiring_scroll()
-> Result<()> {
    let (_directory, mut app) = reader(12)?;
    assert!(app.reader_lines.len() < app.reader_height);
    assert_eq!(percent(&app)?, 0);
    for _ in 0..6 {
        key(&mut app, KeyCode::Char('j'))?;
    }
    assert_eq!(app.scroll, 0);
    assert!((25..75).contains(&percent(&app)?));
    key(&mut app, KeyCode::Char('G'))?;
    assert_eq!(app.scroll, 0);
    assert_eq!(percent(&app)?, 100);
    Ok(())
}

#[test]
fn clicking_a_lower_visible_row_updates_progress_without_scrolling()
-> Result<()> {
    let (_directory, mut app) = reader(80)?;
    let content = ui::workspace_reader_content(SCREEN, &app.workspace);
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: content.x,
        row: content.y + 20,
        modifiers: KeyModifiers::NONE,
    })?;
    assert_eq!(app.reader_cursor, 20);
    assert_eq!(app.scroll, 0);
    assert!(percent(&app)? >= 20);
    app.resize_reader(content.width.into(), 8);
    assert!(percent(&app)? >= 20);
    Ok(())
}
