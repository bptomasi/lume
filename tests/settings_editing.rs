//! Enter edits selected preferences; preview, validation and saving stay distinct.
mod common;
use anyhow::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};
use lume::{
    app::{App, Mode},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn settings(app: &mut App) -> Result<()> {
    key(app, KeyCode::Char(':'))?;
    app.paste("settings")?;
    key(app, KeyCode::Enter)
}

#[test]
fn enter_edits_numeric_values_and_ctrl_s_persists_the_preview() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    settings(&mut app)?;
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Enter)?;
    app.paste("100")?;
    for (width, height) in [(1, 1), (24, 10), (120, 32)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
    }
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.workspace.reading_width, 100);
    assert!(matches!(app.mode, Mode::Settings(_)));
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(
        Store::open(&directory.path().join("library.sqlite3"))?
            .workspace()?
            .reading_width,
        100
    );
    Ok(())
}

#[test]
fn invalid_values_remain_editable_and_escape_cancels_only_the_inner_edit()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    settings(&mut app)?;
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Enter)?;
    app.paste("999")?;
    assert!(key(&mut app, KeyCode::Enter).is_err());
    assert_eq!(app.workspace.reading_width, 82);
    app.mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 40,
        row: 15,
        modifiers: KeyModifiers::NONE,
    })?;
    let Mode::Settings(draft) = &app.mode else {
        anyhow::bail!("Editor closed")
    };
    assert_eq!(draft.field, 1);
    key(&mut app, KeyCode::Esc)?;
    assert!(matches!(app.mode, Mode::Settings(_)));
    key(&mut app, KeyCode::Enter)?;
    app.paste("96")?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.workspace.reading_width, 96);
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.workspace.reading_width, 82);
    Ok(())
}

#[test]
fn theme_values_and_boolean_switches_are_editable_with_enter() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    settings(&mut app)?;
    key(&mut app, KeyCode::Enter)?;
    app.paste("gruvbox")?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.workspace.theme, "gruvbox");
    for _ in 0..4 {
        key(&mut app, KeyCode::Down)?;
    }
    let compact = app.workspace.compact;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.workspace.compact, !compact);
    assert!(matches!(app.mode, Mode::Settings(_)));
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.workspace.theme, "lume");
    assert_eq!(app.workspace.compact, compact);
    Ok(())
}
