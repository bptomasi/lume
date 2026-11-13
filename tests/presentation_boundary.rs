//! Immutable pane paint and interactive capture have separate lifecycles.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use lume::{app::App, frame_composition, model::Pane, ui};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn capture_records_the_themed_base_once_after_immutable_paint() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    common::fixture_command(&mut app, "theme nord")?;
    let screen = Rect::new(0, 0, 160, 44);
    app.resize_workspace(screen);
    let area = hold_reader(&mut app, screen)?;
    assert!(app.panel_drag().is_some());
    assert!(app.floating_pane().is_none());
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    terminal.draw(|frame| ui::draw_base(frame, &app.presentation()))?;
    let base = terminal.backend().buffer().clone();
    terminal.draw(|frame| ui::draw(frame, &app.presentation()))?;
    assert!(app.floating_pane().is_none(), "Paint admitted capture");
    terminal
        .draw(|frame| frame_composition::draw_interactive(frame, &mut app))?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Drag(MouseButton::Left),
        column: area.x + 8,
        row: area.y + 2,
        modifiers: KeyModifiers::NONE,
    })?;
    let picture = app.floating_pane().context("Interactive capture missing")?;
    assert_captured_base(&picture, &base, area);
    let captured = picture.cells.clone();
    change_reader_surface(&mut app, &mut terminal, area, &base)?;
    terminal
        .draw(|frame| frame_composition::draw_interactive(frame, &mut app))?;
    assert_eq!(
        app.floating_pane().context("Captured pane lost")?.cells,
        &captured
    );
    Ok(())
}

fn hold_reader(app: &mut App, screen: Rect) -> Result<Rect> {
    let area = ui::workspace_areas(screen, app.focus(), app.workspace())
        .get(Pane::Reader)
        .context("Reader pane missing")?;
    let grip = ui::pane_move_rect(area).context("Reader grip missing")?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: grip.x + 1,
        row: grip.y,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(area)
}

fn change_reader_surface(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    area: Rect,
    base: &ratatui::buffer::Buffer,
) -> Result<()> {
    app.activate_pane(if app.focus() == Pane::Reader {
        Pane::Library
    } else {
        Pane::Reader
    });
    terminal.draw(|frame| ui::draw_base(frame, &app.presentation()))?;
    assert_ne!(
        terminal.backend().buffer()[(area.x, area.y)],
        base[(area.x, area.y)]
    );
    Ok(())
}

fn assert_captured_base(
    picture: &lume::app::FloatingPane<'_>,
    base: &ratatui::buffer::Buffer,
    area: Rect,
) {
    assert_eq!(picture.pane, Pane::Reader);
    assert_eq!(picture.cells.area.as_size(), area.as_size());
    for row in 0..area.height {
        for column in 0..area.width {
            let source =
                (picture.cells.area.x + column, picture.cells.area.y + row);
            assert_eq!(
                picture.cells[source],
                base[(area.x + column, area.y + row)]
            );
        }
    }
}
