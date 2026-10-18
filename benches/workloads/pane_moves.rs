//! Preview frames and persisted moves, with actual grip and keyboard dispatch.

#[path = "pane_drag.rs"]
mod drag;

use std::time::Instant;

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEventKind,
};
use lume::{
    app::App,
    model::{Pane, Workspace},
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

use super::{SCREEN, dispatch, frame, mouse, report};

pub(super) fn run(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let original = app.workspace.panel_order;
    let mut moved = app.workspace.clone();
    moved.panel_order = [
        Pane::Reader,
        Pane::Library,
        Pane::Articles,
        Pane::Annotations,
    ];
    let outward = gesture(&app.workspace, Pane::Library)?;
    let inward = gesture(&moved, Pane::Articles)?;
    let mut samples = Vec::new();
    for _ in 0..iterations {
        let first = outward.clone();
        let second = inward.clone();
        let start = Instant::now();
        move_frame(app, terminal, first)?;
        let intermediate = app.workspace.panel_order;
        move_frame(app, terminal, second)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        ensure!(
            intermediate == moved.panel_order,
            "Grip move lost its destination"
        );
        ensure!(
            app.workspace.panel_order == original,
            "Grip roundtrip changed order"
        );
    }
    report(
        "two grip moves (128 motion events) + preview/commit frames",
        samples,
    );
    keyboard_workload(app, terminal, iterations)?;
    drag::run(app, terminal, iterations)
}

fn gesture(
    workspace: &Workspace,
    target: Pane,
) -> Result<(Vec<Event>, Vec<Event>)> {
    let areas = ui::workspace_areas(SCREEN, Pane::Reader, workspace);
    let reader = areas.reader.context("Reader hidden")?;
    let grip = ui::pane_move_rect(reader).context("Reader grip hidden")?;
    let destination = areas.get(target).context("Destination hidden")?;
    let left = if destination.x < reader.x {
        destination.x
    } else {
        destination.right().saturating_sub(reader.width)
    };
    let column = grip.x + left - reader.x;
    let mut events = vec![mouse(
        MouseEventKind::Down(MouseButton::Left),
        grip.x,
        grip.y,
    )];
    events.extend(
        (0..64).map(|_| {
            mouse(MouseEventKind::Drag(MouseButton::Left), column, 12)
        }),
    );
    let release = mouse(MouseEventKind::Up(MouseButton::Left), column, 12);
    Ok((events, vec![release]))
}

fn move_frame(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    (preview, release): (Vec<Event>, Vec<Event>),
) -> Result<()> {
    dispatch(app, preview)?;
    frame(app, terminal)?;
    dispatch(app, release)?;
    frame(app, terminal)
}

fn keyboard_workload(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let original = app.workspace.panel_order;
    let expected = [
        Pane::Library,
        Pane::Reader,
        Pane::Articles,
        Pane::Annotations,
    ];
    let mut samples = Vec::new();
    for _ in 0..iterations * 5 {
        let start = Instant::now();
        app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT))?;
        frame(app, terminal)?;
        let intermediate = app.workspace.panel_order;
        app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT))?;
        frame(app, terminal)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        ensure!(
            intermediate == expected,
            "Keyboard move did not change visible order"
        );
        ensure!(
            app.workspace.panel_order == original,
            "Keyboard roundtrip lost order"
        );
    }
    report("two keyboard moves + persisted layout/frames", samples);
    Ok(())
}
