//! The guide's search stays local and preserves navigation and cancellation.
mod common;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode},
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn screen(app: &App, width: u16, height: u16) -> Result<String> {
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    Ok(terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol())
        .collect())
}

#[test]
fn live_help_search_moves_between_matches_and_esc_clears_before_closing()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let article = app.current().context("Article")?.id;
    key(&mut app, KeyCode::Char('?'))?;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("resize")?;
    let first = app.help_scroll;
    assert!(screen(&app, 160, 44)?.contains("matches"));
    key(&mut app, KeyCode::Enter)?;
    key(&mut app, KeyCode::Char('n'))?;
    assert_ne!(app.help_scroll, first);
    key(&mut app, KeyCode::Char('N'))?;
    assert_eq!(app.help_scroll, first);
    key(&mut app, KeyCode::Esc)?;
    assert!(matches!(app.mode, Mode::Help));
    assert!(!screen(&app, 160, 44)?.contains("matches · n/N"));
    key(&mut app, KeyCode::Esc)?;
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(app.current().context("Selection lost")?.id, article);
    assert!(app.search.is_empty());
    Ok(())
}

#[test]
fn cancelled_help_query_restores_previous_match_and_large_paste_is_bounded()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    key(&mut app, KeyCode::Char('?'))?;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("resize")?;
    key(&mut app, KeyCode::Enter)?;
    key(&mut app, KeyCode::Char('n'))?;
    let previous = app.help_scroll;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste(&"不存在\u{1b}\n".repeat(2000))?;
    assert!(screen(&app, 160, 44)?.contains("0/0 matches"));
    for (width, height) in [(1, 1), (20, 4), (80, 24)] {
        screen(&app, width, height)?;
    }
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.help_scroll, previous);
    assert!(screen(&app, 160, 44)?.contains("resize"));
    Ok(())
}
