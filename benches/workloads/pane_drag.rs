//! Real held-pane frames and elapsed-time settling, without pacing sleeps.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::{App, FloatingPane},
    model::Pane,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

use super::super::{SCREEN, frame, report};

pub(super) fn run(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let scenario = DragScenario::new(app)?;
    let original = app.workspace.panel_order;
    let text = app.reader_text.clone();
    let mut captures = Vec::new();
    let mut held = Vec::new();
    let mut settling = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        app.mouse(scenario.down)?;
        frame(app, terminal)?;
        captures.push(start.elapsed().as_secs_f64() * 1000.0);
        for (motion, expected) in scenario.motions {
            let start = Instant::now();
            app.mouse(motion)?;
            frame(app, terminal)?;
            held.push(start.elapsed().as_secs_f64() * 1000.0);
            verify_held_picture(
                app,
                terminal.backend().buffer(),
                (expected, motion.row),
            )?;
        }
        settling.extend(release_frames(app, terminal, scenario.release)?);
        ensure!(app.workspace.panel_order == original, "Settling lost order");
        ensure!(app.reader_text == text, "Motion changed readable text");
    }
    report("pane capture + initial runtime frame", captures);
    report("held-pane motion + frozen-cell runtime frame", held);
    report("settling tick + frozen-cell runtime frame", settling);
    neighbor_preview(app, terminal, iterations)?;
    compact_preview(app, terminal, iterations)
}

fn compact_preview(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let workspace = app.workspace.clone();
    let focus = app.focus;
    let mut samples = Vec::new();
    for _ in 0..iterations {
        app.workspace.fullscreen = true;
        app.focus = Pane::Reader;
        app.resize_workspace(SCREEN);
        let grip =
            ui::pane_move_rect(SCREEN).context("Fullscreen grip hidden")?;
        let start = Instant::now();
        app.mouse(event(
            MouseEventKind::Down(MouseButton::Left),
            grip.x,
            grip.y,
        ))?;
        frame(app, terminal)?;
        app.mouse(event(
            MouseEventKind::Drag(MouseButton::Left),
            SCREEN.width / 2,
            5,
        ))?;
        frame(app, terminal)?;
        let card = app.floating_pane().context("Summary card missing")?;
        ensure!(
            card.area.width <= 38 && card.area.height <= 9,
            "Oversized capture did not use a bounded summary"
        );
        verify_rounded_outline(&card, terminal.backend().buffer())?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    }
    app.workspace = workspace;
    app.focus = focus;
    app.resize_workspace(SCREEN);
    report("compact pane capture + drag + two runtime frames", samples);
    Ok(())
}

fn neighbor_preview(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let scenario = DragScenario::new(app)?;
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let reader = areas.reader.context("Reader hidden")?;
    let article = areas.articles.context("Articles hidden")?;
    let order = app.workspace.panel_order;
    let mut samples = Vec::new();
    for _ in 0..iterations {
        app.mouse(scenario.down)?;
        frame(app, terminal)?;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Drag(MouseButton::Left),
            ..scenario.release
        })?;
        let now = Instant::now();
        for step in 1..=10 {
            let start = Instant::now();
            app.tick_pane_motion(now + Duration::from_millis(step * 16));
            frame(app, terminal)?;
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        verify_insertion(app, article, reader, order)?;
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
        frame(app, terminal)?;
    }
    report(
        "neighbor insertion tick + frozen-cell runtime frame",
        samples,
    );
    Ok(())
}

fn verify_insertion(
    app: &App,
    article: Rect,
    reader: Rect,
    order: [Pane; 4],
) -> Result<()> {
    let neighbor = app
        .neighboring_panes()
        .find(|item| item.pane == Pane::Articles)
        .context("Missing neighbor")?;
    ensure!(
        neighbor.area.x == article.x + reader.width,
        "Neighbor did not open the insertion slot"
    );
    ensure!(
        app.pane_insertion_slot()
            .context("Missing insertion slot")?
            .x
            == article.x,
        "Overlap selected the wrong insertion slot"
    );
    ensure!(
        app.workspace.panel_order == order,
        "Preview changed saved order"
    );
    Ok(())
}

fn verify_held_picture(
    app: &App,
    buffer: &Buffer,
    expected: (u16, u16),
) -> Result<()> {
    let picture = app.floating_pane().context("Held picture missing")?;
    ensure!(
        (picture.area.x, picture.area.y) == expected,
        "Picture did not follow the pointer's grab offset"
    );
    ensure!(
        buffer[(0, 0)].bg == ratatui::style::Color::Rgb(13, 13, 15),
        "Held backdrop was not dimmed"
    );
    verify_rounded_outline(&picture, buffer)
}

fn verify_rounded_outline(
    picture: &FloatingPane<'_>,
    buffer: &Buffer,
) -> Result<()> {
    let source = picture.cells.area;
    for (column, row, symbol) in [
        (0, 0, "╭"),
        (source.width - 1, 0, "╮"),
        (0, source.height - 1, "╰"),
        (source.width - 1, source.height - 1, "╯"),
    ] {
        ensure!(
            picture.cells[(source.x + column, source.y + row)].symbol()
                == symbol,
            "Capture lost a rounded corner"
        );
        if let Some(cell) =
            buffer.cell((picture.area.x + column, picture.area.y + row))
        {
            ensure!(
                cell.symbol() == symbol,
                "Visible corner did not move with the pane"
            );
        }
    }
    Ok(())
}

struct DragScenario {
    down: MouseEvent,
    motions: [(MouseEvent, u16); 5],
    release: MouseEvent,
}

impl DragScenario {
    fn new(app: &App) -> Result<Self> {
        let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
        let reader = areas.reader.context("Reader hidden")?;
        let grip = ui::pane_move_rect(reader).context("Reader grip hidden")?;
        let down =
            event(MouseEventKind::Down(MouseButton::Left), grip.x + 1, grip.y);
        let motions = [12, 8, 20, 4, 16].map(|delta| {
            (
                event(
                    MouseEventKind::Drag(MouseButton::Left),
                    grip.x + 1 - delta,
                    delta,
                ),
                reader.x - delta,
            )
        });
        // The panel overlaps Articles while the grip remains inside Reader.
        let release =
            event(MouseEventKind::Up(MouseButton::Left), grip.x + 1 - 12, 12);
        Ok(Self {
            down,
            motions,
            release,
        })
    }
}

fn release_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    release: MouseEvent,
) -> Result<Vec<f64>> {
    app.mouse(release)?;
    ensure!(app.pane_motion_active(), "Release failed to start settling");
    ensure!(
        app.workspace.panel_order
            == [
                Pane::Library,
                Pane::Reader,
                Pane::Articles,
                Pane::Annotations
            ],
        "Release committed the wrong order"
    );
    let now = Instant::now();
    let mut samples = Vec::new();
    for step in 1..=10 {
        let start = Instant::now();
        app.tick_pane_motion(now + Duration::from_millis(step * 16));
        frame(app, terminal)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    ensure!(!app.pane_motion_active(), "Animation did not finish");
    app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT))?;
    frame(app, terminal)?;
    Ok(samples)
}

fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}
