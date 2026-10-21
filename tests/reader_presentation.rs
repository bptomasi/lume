//! Presentation preferences preserve source bytes and isolate block viewports.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus},
    source, ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn reading(body: &str) -> Result<(tempfile::TempDir, App, Rect)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.workspace.fullscreen_width = 40;
    let screen = Rect::new(0, 0, 100, 30);
    app.resize_workspace(screen);
    Ok((directory, app, screen))
}

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

#[test]
fn justified_prose_keeps_copy_quotes_and_saved_preference_exact() -> Result<()>
{
    let body = "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima.";
    let (directory, mut app, screen) = reading(body)?;
    let original = app.reader_text.clone();
    command(&mut app, "justify on")?;
    let mut terminal =
        Terminal::new(TestBackend::new(screen.width, screen.height))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let content = ui::workspace_reader_content(screen, &app.workspace);
    let visible = (content.x..content.right())
        .map(|x| terminal.backend().buffer()[(x, content.y)].symbol())
        .collect::<String>();
    assert_eq!(visible, "Alpha  bravo  charlie delta echo foxtrot");
    assert_eq!(app.reader_text, original);
    for code in ['w', 'v', '4', 'l'] {
        key(&mut app, KeyCode::Char(code))?;
    }
    assert_eq!(
        key(&mut app, KeyCode::Char('y'))?,
        Action::Copy("bravo".into())
    );
    key(&mut app, KeyCode::Char('s'))?;
    assert_eq!(
        app.comments.last().context("Highlight missing")?.quote,
        "bravo"
    );
    assert_eq!(app.body, body);
    let store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(store.workspace()?.justify);
    Ok(())
}

#[test]
fn scrolling_one_code_block_does_not_shift_other_blocks() -> Result<()> {
    let body = format!(
        "```sql\nSELECT FIRST_BLOCK_{};\n```\n\n```sql\nSELECT SECOND_BLOCK_{};\n```",
        "a".repeat(120),
        "b".repeat(120)
    );
    let (_directory, mut app, screen) = reading(&body)?;
    key(&mut app, KeyCode::Char('$'))?;
    assert!(app.horizontal > 0);
    let first_offset = app.horizontal;
    let mut terminal =
        Terminal::new(TestBackend::new(screen.width, screen.height))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let visible = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(visible.contains("SECOND_BLOCK_"));
    command(&mut app, "find SECOND_BLOCK_")?;
    assert_eq!(app.horizontal, 0);
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(app.horizontal, first_offset);
    Ok(())
}

#[test]
fn horizontal_mouse_wheel_changes_only_the_block_under_the_pointer()
-> Result<()> {
    use crossterm::event::{MouseEvent, MouseEventKind};
    let body = format!(
        "```sql\nSELECT FIRST_BLOCK_{};\n```\n\n```sql\nSELECT SECOND_BLOCK_{};\n```",
        "a".repeat(120),
        "b".repeat(120)
    );
    let (_directory, mut app, screen) = reading(&body)?;
    let content = ui::workspace_reader_content(screen, &app.workspace);
    let row = app
        .reader_lines
        .iter()
        .position(|line| line.contains("SECOND_BLOCK_"))
        .context("Second code block missing")?;
    for _ in 0..4 {
        app.mouse(MouseEvent {
            kind: MouseEventKind::ScrollRight,
            column: content.x + 4,
            row: content.y + row as u16,
            modifiers: KeyModifiers::NONE,
        })?;
    }
    assert_eq!(app.horizontal, 0);
    command(&mut app, "find SECOND_BLOCK_")?;
    assert_eq!(app.horizontal, 4);
    command(&mut app, "find FIRST_BLOCK_")?;
    assert_eq!(app.horizontal, 0);
    Ok(())
}

#[test]
fn mouse_clicks_use_original_bytes_inside_justified_prose() -> Result<()> {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let (_directory, mut app, screen) = reading(
        "Alpha bravo charlie delta echo foxtrot golf hotel india juliet kilo lima.",
    )?;
    command(&mut app, "justify on")?;
    let content = ui::workspace_reader_content(screen, &app.workspace);
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        app.mouse(MouseEvent {
            kind,
            column: content.x + 7,
            row: content.y,
            modifiers: KeyModifiers::NONE,
        })?;
    }
    assert_eq!(app.cursor_offset(), 6);
    assert!(app.reader_text[app.cursor_offset()..].starts_with("bravo"));
    Ok(())
}

#[test]
fn justification_excludes_structural_rows_and_final_paragraph_lines() {
    let prose = "Alpha bravo charlie delta echo foxtrot golf hotel india.";
    let rendered = lume::reader::render(prose, 30);
    let eligible = rendered
        .lines
        .iter()
        .zip(&rendered.justifiable)
        .filter(|(_, eligible)| **eligible)
        .map(|(line, _)| line.to_string())
        .collect::<Vec<_>>();
    assert_eq!(eligible, ["Alpha bravo charlie delta echo"]);

    for source in [
        format!("# {prose}"),
        format!("- {prose}"),
        format!("> {prose}"),
        format!("```text\n{prose}\n```"),
        "| Name | Value |\n| --- | --- |\n| Alpha | Bravo |".into(),
    ] {
        let structural = lume::reader::render(&source, 30);
        assert!(
            structural.justifiable.iter().all(|eligible| !eligible),
            "Structural text was marked for justification: {source}"
        );
    }
}
