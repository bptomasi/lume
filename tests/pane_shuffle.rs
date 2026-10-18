//! Live insertion previews remain transient, bounded and interruptible.

mod common;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{app::App, model::Pane, ui};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 160, 44);

fn application() -> Result<(tempfile::TempDir, App)> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn mouse(
    app: &mut App,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> Result<()> {
    app.mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}

fn frame(app: &mut App) -> Result<()> {
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    terminal.draw(|frame| ui::draw_interactive(frame, app))?;
    Ok(())
}

fn begin(app: &mut App, pane: Pane) -> Result<ui::Areas> {
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let grip = ui::pane_move_rect(areas.get(pane).context("Pane hidden")?)
        .context("Grip hidden")?;
    mouse(
        app,
        MouseEventKind::Down(MouseButton::Left),
        grip.x + 1,
        grip.y,
    )?;
    frame(app)?;
    Ok(areas)
}

fn neighbor(app: &App, pane: Pane) -> Result<Rect> {
    app.neighboring_panes()
        .find(|item| item.pane == pane)
        .map(|item| item.area)
        .context("Neighbor missing")
}

#[test]
fn neighbors_make_room_while_saved_order_stays_unchanged() -> Result<()> {
    let (directory, mut app) = application()?;
    let order = app.workspace.panel_order;
    let areas = begin(&mut app, Pane::Library)?;
    let reader = areas.reader.context("Reader hidden")?;
    let article = areas.articles.context("Articles hidden")?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    let now = Instant::now();

    app.tick_pane_motion(now + Duration::from_millis(80));

    assert!(neighbor(&app, Pane::Articles)?.x < article.x);
    assert!(neighbor(&app, Pane::Articles)?.x > 0);
    assert!(app.pane_motion_active());
    app.tick_pane_motion(now + Duration::from_secs(1));
    assert_eq!(neighbor(&app, Pane::Articles)?.x, 0);
    assert_eq!(neighbor(&app, Pane::Reader)?.x, article.width);
    assert_eq!(
        app.pane_insertion_slot().context("Gap missing")?.x,
        article.width + reader.width
    );
    assert!(!app.pane_motion_active());
    assert_eq!(app.workspace.panel_order, order);
    let store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.workspace()?.panel_order, order);
    Ok(())
}

#[test]
fn crossing_back_retargets_the_neighbors_and_escape_restores_every_pane()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Library)?;
    let reader = areas.reader.context("Reader hidden")?;
    let article = areas.articles.context("Articles hidden")?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    app.tick_pane_motion(Instant::now() + Duration::from_millis(80));
    let before = neighbor(&app, Pane::Articles)?;

    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 10, 12)?;

    assert_eq!(neighbor(&app, Pane::Articles)?, before);
    app.tick_pane_motion(Instant::now() + Duration::from_secs(1));
    assert_eq!(neighbor(&app, Pane::Articles)?, article);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.neighboring_panes().next().is_none());
    assert!(app.pane_insertion_slot().is_none());
    assert!(!app.pane_motion_active());
    Ok(())
}

#[test]
fn release_continues_from_preview_and_commits_the_same_insertion_order()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Library)?;
    let reader = areas.reader.context("Reader hidden")?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    app.tick_pane_motion(Instant::now() + Duration::from_millis(80));
    let before = neighbor(&app, Pane::Articles)?;

    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        reader.x + 4,
        12,
    )?;

    assert_eq!(neighbor(&app, Pane::Articles)?, before);
    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Articles,
            Pane::Reader,
            Pane::Library,
            Pane::Annotations
        ]
    );
    app.tick_pane_motion(Instant::now() + Duration::from_secs(1));
    assert!(!app.pane_motion_active());
    assert!(app.neighboring_panes().next().is_none());
    assert!(app.floating_pane().is_none());
    Ok(())
}

#[test]
fn a_stationary_held_pane_stays_visible_while_neighbors_return() -> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Library)?;
    let reader = areas.reader.context("Reader hidden")?;
    let grip = ui::pane_move_rect(areas.library.context("Library hidden")?)
        .context("Grip hidden")?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    app.tick_pane_motion(Instant::now() + Duration::from_millis(80));

    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        grip.x + 1,
        grip.y,
    )?;

    assert!(app.pane_motion_active());
    let held = app.floating_pane().context("Stationary picture missing")?;
    assert_eq!(held.area, held.slot);
    frame(&mut app)?;
    app.tick_pane_motion(Instant::now() + Duration::from_secs(1));
    assert!(!app.pane_motion_active());
    assert!(app.floating_pane().is_none());
    Ok(())
}

#[test]
fn resize_cancels_live_neighbor_motion_before_reflow() -> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Library)?;
    let reader = areas.reader.context("Reader hidden")?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    assert!(app.pane_motion_active());

    app.resize_workspace(Rect::new(0, 0, 80, 24));

    assert!(app.neighboring_panes().next().is_none());
    assert!(app.pane_insertion_slot().is_none());
    assert!(!app.pane_motion_active());
    assert_eq!(app.workspace.panel_order, Pane::ALL);
    Ok(())
}

#[test]
fn pane_overlap_opens_a_slot_before_the_grip_reaches_the_neighbor() -> Result<()>
{
    let (directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Reader)?;
    let reader = areas.reader.context("Reader hidden")?;
    let article = areas.articles.context("Articles hidden")?;
    let grip = ui::pane_move_rect(reader).context("Grip hidden")?;
    let column = grip.x + 1 - 12;
    assert!(column >= reader.x);

    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        column,
        12,
    )?;

    assert_eq!(
        app.panel_drag().context("Preview missing")?.target,
        Some(Pane::Articles)
    );
    app.tick_pane_motion(Instant::now() + Duration::from_secs(1));
    assert_eq!(neighbor(&app, Pane::Articles)?.x, article.x + reader.width);
    assert_eq!(
        app.pane_insertion_slot().context("Gap missing")?.x,
        article.x
    );
    assert_eq!(app.workspace.panel_order, Pane::ALL);
    let store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.workspace()?.panel_order, Pane::ALL);
    Ok(())
}

#[test]
fn dropping_while_the_grip_is_still_in_the_source_commits_the_preview()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Reader)?;
    let reader = areas.reader.context("Reader hidden")?;
    let grip = ui::pane_move_rect(reader).context("Grip hidden")?;
    let column = grip.x + 1 - 12;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        column,
        12,
    )?;

    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), column, 12)?;

    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Library,
            Pane::Reader,
            Pane::Articles,
            Pane::Annotations
        ]
    );
    Ok(())
}

#[test]
fn title_drag_can_open_the_right_neighbor_before_the_pointer_crosses_it()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.workspace.show_annotations = true;
    app.resize_workspace(SCREEN);
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let reader = areas.reader.context("Reader hidden")?;
    let notes = areas.annotations.context("Notes hidden")?;
    let column = reader.x + 4;
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        column,
        reader.y,
    )?;
    frame(&mut app)?;
    assert!(column + 12 < notes.x);

    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        column + 12,
        12,
    )?;

    assert_eq!(
        app.panel_drag().context("Preview missing")?.target,
        Some(Pane::Annotations)
    );
    app.tick_pane_motion(Instant::now() + Duration::from_secs(1));
    assert_eq!(neighbor(&app, Pane::Annotations)?.x, reader.x);
    assert_eq!(app.workspace.panel_order, Pane::ALL);
    Ok(())
}

#[test]
fn small_pointer_reversals_keep_the_preview_until_the_retention_margin_is_crossed()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = begin(&mut app, Pane::Reader)?;
    let reader = areas.reader.context("Reader hidden")?;
    let grip = ui::pane_move_rect(reader).context("Grip hidden")?;
    for (delta, target) in [
        (12, Some(Pane::Articles)),
        (10, Some(Pane::Articles)),
        (6, Some(Pane::Articles)),
        (5, None),
        (10, None),
        (12, Some(Pane::Articles)),
    ] {
        mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            grip.x + 1 - delta,
            12,
        )?;
        assert_eq!(app.panel_drag().context("Preview missing")?.target, target);
    }
    Ok(())
}
