//! Checked-in cell snapshots cover geometry, colors and selection across themes.
//!
//! UPDATE_LUME_VISUALS=1 explicitly regenerates fixtures for visual review.
//! Ordinary tests only compare them; they never accept changed output.
//! Fixture rows retain exact terminal cells, including widths beyond 80 columns.

mod common;

use std::{fmt::Write, path::Path};

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{app::App, model::Pane, ui};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

#[test]
fn layouts_guide_selection_and_drag_match_reviewed_cells() -> Result<()> {
    for theme in ["lume", "nord", "gruvbox"] {
        for (width, height) in [(144, 44), (80, 28), (40, 18)] {
            for scene in ["split", "guide", "selection", "drag"] {
                let buffer = scenario(theme, width, height, scene)?;
                let actual = snapshot(&buffer)?;
                let name = format!("{theme}-{width}x{height}-{scene}.txt");
                let path = Path::new("tests/fixtures/ui").join(name);
                if std::env::var_os("UPDATE_LUME_VISUALS").is_some() {
                    std::fs::create_dir_all(
                        path.parent().context("No fixture parent")?,
                    )?;
                    std::fs::write(&path, &actual)?;
                }
                let expected =
                    std::fs::read_to_string(&path).with_context(|| {
                        format!("Missing reviewed snapshot: {}", path.display())
                    })?;
                assert_eq!(
                    actual,
                    expected,
                    "Changed scene: {}",
                    path.display()
                );
            }
        }
    }
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn scenario(
    theme: &str,
    width: u16,
    height: u16,
    scene: &str,
) -> Result<Buffer> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "workspace load Study")?;
    app.workspace.theme = theme.into();
    app.focus = Pane::Reader;
    app.status = "Visual regression fixture".into();
    let area = Rect::new(0, 0, width, height);
    app.resize_workspace(area);
    configure(&mut app, area, scene)?;
    let mut terminal = Terminal::new(TestBackend::new(width, height))?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    Ok(terminal.backend().buffer().clone())
}

fn configure(app: &mut App, area: Rect, scene: &str) -> Result<()> {
    match scene {
        "guide" => {
            app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
        }
        "selection" => {
            app.key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))?;
            app.key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE))?;
        }
        "drag" => {
            let reader = ui::workspace_areas(area, app.focus, &app.workspace)
                .reader
                .context("Reader hidden")?;
            let grip = ui::pane_move_rect(reader).context("Grip hidden")?;
            for (kind, x, y) in [
                (MouseEventKind::Down(MouseButton::Left), grip.x, grip.y),
                (
                    MouseEventKind::Drag(MouseButton::Left),
                    grip.x.saturating_sub(3),
                    grip.y + 2,
                ),
            ] {
                app.mouse(MouseEvent {
                    kind,
                    column: x,
                    row: y,
                    modifiers: KeyModifiers::NONE,
                })?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn snapshot(buffer: &Buffer) -> Result<String> {
    let mut text = String::new();
    for y in buffer.area.y..buffer.area.bottom() {
        for x in buffer.area.x..buffer.area.right() {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    // Style runs retain every cell's colors/modifiers without per-cell noise.
    for y in buffer.area.y..buffer.area.bottom() {
        let mut x = buffer.area.x;
        while x < buffer.area.right() {
            let start = x;
            let style = buffer[(x, y)].style();
            while x < buffer.area.right() && buffer[(x, y)].style() == style {
                x += 1;
            }
            writeln!(&mut text, "{y}:{start}..{x} {style:?}")?;
        }
    }
    Ok(text)
}
