//! Wheel events retain their reported distance without application acceleration.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};
use lume::{
    app::{App, InputBatch, Mode},
    model::Pane,
    ui,
};
use ratatui::layout::Rect;

const SCREEN: Rect = Rect::new(0, 0, 160, 30);

fn event(kind: MouseEventKind, x: u16, y: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    }
}

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn reader() -> Result<(tempfile::TempDir, App, Rect)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = (0..100)
        .map(|index| format!("Distinct paragraph {index}.\n\n"))
        .collect();
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "fullscreen")?;
    let content = ui::workspace_reader_content(SCREEN, &app.workspace);
    Ok((directory, app, content))
}

#[test]
fn individual_wheel_steps_scroll_exactly_one_visual_line_and_stop_at_edges()
-> Result<()> {
    let (_directory, mut app, content) = reader()?;
    for expected in 1..=10 {
        app.mouse(event(MouseEventKind::ScrollDown, content.x, content.y))?;
        assert_eq!(app.scroll, expected);
    }
    for expected in (0..10).rev() {
        app.mouse(event(MouseEventKind::ScrollUp, content.x, content.y))?;
        assert_eq!(app.scroll, expected);
    }
    app.mouse(event(MouseEventKind::ScrollUp, content.x, content.y))?;
    assert_eq!(app.scroll, 0);
    let events = vec![
        Event::Mouse(event(
            MouseEventKind::ScrollDown,
            content.x,
            content.y,
        ));
        400
    ];
    let mut batch = InputBatch::new(events);
    batch.dispatch_next(&mut app)?;
    assert_eq!(app.scroll, app.reader_lines.len() - app.reader_height);
    Ok(())
}

#[test]
fn wheel_bursts_preserve_distance_and_direction_without_panning_prose()
-> Result<()> {
    let (_directory, mut app, content) = reader()?;
    let mut events = vec![
        Event::Mouse(event(
            MouseEventKind::ScrollDown,
            content.x,
            content.y,
        ));
        8
    ];
    events.push(Event::Mouse(event(
        MouseEventKind::ScrollUp,
        content.x,
        content.y,
    )));
    let mut batch = InputBatch::new(events);
    batch.dispatch_next(&mut app)?;
    assert_eq!(app.scroll, 8);
    batch.dispatch_next(&mut app)?;
    assert_eq!(app.scroll, 7);
    assert!(batch.is_empty());

    app.mouse(event(MouseEventKind::ScrollRight, content.x, content.y))?;
    assert_eq!(app.horizontal, 0);
    app.mouse(event(MouseEventKind::ScrollLeft, content.x, content.y))?;
    assert_eq!(app.horizontal, 0);
    assert_eq!(app.scroll, 7);
    Ok(())
}

#[test]
fn sidebar_wheel_steps_select_one_destination_instead_of_skipping_items()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    let before = app.nav_selected;
    app.mouse(event(MouseEventKind::ScrollDown, 4, 12))?;
    assert_eq!(app.focus, Pane::Library);
    assert_eq!(app.nav_selected, before + 1);
    app.mouse(event(MouseEventKind::ScrollUp, 4, 12))?;
    assert_eq!(app.nav_selected, before);
    Ok(())
}

#[test]
fn modal_scroll_is_one_step_and_reaches_the_fullscreen_width_preview()
-> Result<()> {
    let (_directory, mut app, _) = reader()?;
    command(&mut app, "help")?;
    app.mouse(event(MouseEventKind::ScrollDown, 80, 15))?;
    assert_eq!(app.help_scroll, 1);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    command(&mut app, "settings")?;
    let mut batch = InputBatch::new(vec![
        Event::Mouse(event(
            MouseEventKind::ScrollDown,
            80,
            15,
        ));
        11
    ]);
    batch.dispatch_next(&mut app)?;
    let Mode::Settings(draft) = &app.mode else {
        anyhow::bail!("Settings preview closed unexpectedly");
    };
    assert_eq!(draft.field, 11);
    app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))?;
    assert_eq!(app.workspace.fullscreen_width, 81);
    app.resize_workspace(SCREEN);
    assert_eq!(app.reader_width, 81);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert_eq!(app.workspace.fullscreen_width, 80);
    app.resize_workspace(SCREEN);
    assert_eq!(app.reader_width, 80);
    assert!(app.current().context("Reader lost its article")?.unread);
    Ok(())
}
