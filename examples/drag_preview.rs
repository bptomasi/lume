//! Export actual held and settling frames for an animated visual review.

mod support;

use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{app::App, demo, store::Store, ui};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 144, 44);

// Leftward displacement and pointer row: the grip stays inside Reader.
const HELD_PATH: [(u16, u16); 9] = [
    (2, 4),
    (4, 6),
    (6, 8),
    (8, 10),
    (10, 12),
    (12, 12),
    (14, 10),
    (18, 8),
    (20, 6),
];

fn main() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("/tmp/lume-drag-frames"), PathBuf::from);
    fs::create_dir_all(&output)?;
    let (_directory, mut app) = application()?;
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let grip = ui::pane_move_rect(areas.reader.context("Reader hidden")?)
        .context("Grip hidden")?;
    let down =
        mouse(MouseEventKind::Down(MouseButton::Left), grip.x + 1, grip.y);
    app.mouse(down)?;
    let mut frame = 0;
    export(&mut app, &mut terminal, &output, &mut frame)?;
    for (offset, row) in HELD_PATH {
        let column = grip.x + 1 - offset;
        app.mouse(mouse(MouseEventKind::Drag(MouseButton::Left), column, row))?;
        let now = Instant::now();
        for millis in [0, 48, 96, 176] {
            app.tick_pane_motion(now + Duration::from_millis(millis));
            export(&mut app, &mut terminal, &output, &mut frame)?;
        }
    }
    app.mouse(mouse(
        MouseEventKind::Up(MouseButton::Left),
        grip.x + 1 - 20,
        6,
    ))?;
    let now = Instant::now();
    for step in 0..=10 {
        app.tick_pane_motion(now + Duration::from_millis(step * 16));
        export(&mut app, &mut terminal, &output, &mut frame)?;
    }
    println!("{} frames in {}", frame, output.display());
    Ok(())
}

fn application() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("preview.sqlite3"))?;
    demo::seed(&mut store)?;
    let mut app = App::new(store, true)?;
    command(&mut app, "articles")?;
    command(&mut app, "annotations")?;
    command(&mut app, "panel-focus reader")?;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn command(app: &mut App, command: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(command)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn export(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    output: &std::path::Path,
    frame: &mut usize,
) -> Result<()> {
    app.poll_reader();
    terminal.draw(|frame| ui::draw_interactive(frame, app))?;
    support::export(
        terminal.backend().buffer(),
        &output.join(format!("{frame:03}.json")),
    )?;
    *frame += 1;
    Ok(())
}
