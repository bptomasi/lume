//! Split geometry, profiles and history retain existing libraries and selection.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::App,
    model::{Dock, Pane, PaneLayout, PanelLayout},
    store::Store,
    ui,
};
use ratatui::layout::Rect;

const SCREEN: Rect = Rect::new(0, 0, 160, 50);

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn mouse(app: &mut App, kind: MouseEventKind, x: u16, y: u16) -> Result<()> {
    app.mouse(MouseEvent {
        kind,
        column: x,
        row: y,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}

#[test]
fn split_click_and_drag_round_trip_preserve_the_exact_saved_share() -> Result<()>
{
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let screen = Rect::new(0, 0, 161, 50);
    app.resize_workspace(screen);
    command(&mut app, "workspace load Study")?;
    let before = PanelLayout::capture(&app.workspace);
    let seam = ui::workspace_areas(screen, app.focus, &app.workspace)
        .boundaries
        .into_iter()
        .find(|seam| seam.horizontal)
        .context("Missing horizontal split")?;
    let x = seam.second.x;
    let y = seam.first.y + 3;
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x, y)?;
    assert_eq!(PanelLayout::capture(&app.workspace), before);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x + 5, y)?;
    assert_ne!(PanelLayout::capture(&app.workspace), before);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x, y)?;
    assert_eq!(PanelLayout::capture(&app.workspace), before);
    Ok(())
}

#[test]
fn keyboard_resize_reverses_a_clamped_split_without_invisible_steps()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let screen = Rect::new(0, 0, 100, 50);
    app.workspace.layout = Some(PaneLayout::columns(&Pane::ALL));
    app.workspace
        .layout
        .as_mut()
        .context("Layout")?
        .resize_path(&[], 85);
    app.focus = Pane::Library;
    app.resize_workspace(screen);
    app.save_session()?;
    let before = ui::workspace_areas(screen, app.focus, &app.workspace)
        .library
        .context("Library hidden")?
        .width;
    command(&mut app, "panel-resize -2")?;
    let after = ui::workspace_areas(screen, app.focus, &app.workspace)
        .library
        .context("Library hidden")?
        .width;
    assert!(after < before, "Saved percentage hid the first resize step");
    command(&mut app, "layout-undo")?;
    assert_eq!(
        ui::workspace_areas(screen, app.focus, &app.workspace)
            .library
            .context("Library hidden")?
            .width,
        before
    );
    let original = PanelLayout::capture(&app.workspace);
    command(&mut app, "panel-resize 0")?;
    assert_eq!(PanelLayout::capture(&app.workspace), original);
    command(&mut app, "panel-focus reader")?;
    command(&mut app, "panel-resize -32768")?;
    command(&mut app, "panel-resize 32767")?;
    assert!(app.workspace.layout.as_ref().is_some_and(PaneLayout::valid));
    Ok(())
}

#[test]
fn focusing_an_already_visible_pane_does_not_write_layout_history() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    app.save_session()?;
    let injector =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    injector.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_layout_history.sql"
    ))?;
    command(&mut app, "panel-focus reader")?;
    command(&mut app, "panel-focus articles")?;
    assert!(command(&mut app, "panel annotations").is_err());
    Ok(())
}

#[test]
fn classic_dividers_follow_the_pointer_when_preferred_widths_are_clamped()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let screen = Rect::new(0, 0, 120, 40);
    app.workspace.article_width = 80;
    app.workspace.library_width = 48;
    app.resize_workspace(screen);
    app.save_session()?;
    let before = PanelLayout::capture(&app.workspace);
    let articles = ui::workspace_areas(screen, app.focus, &app.workspace)
        .articles
        .context("Articles hidden")?;
    let x = articles.right();
    let y = articles.y + 3;
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x - 2, y)?;
    let changed = ui::workspace_areas(screen, app.focus, &app.workspace)
        .articles
        .context("Articles hidden")?;
    assert_eq!(changed.right(), x - 2);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x, y)?;
    assert_eq!(PanelLayout::capture(&app.workspace), before);
    Ok(())
}

#[test]
fn dragging_one_classic_divider_keeps_other_pane_width_preferences()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let screen = Rect::new(0, 0, 120, 40);
    app.workspace.show_annotations = true;
    app.workspace.article_width = 80;
    app.workspace.annotation_width = 80;
    app.resize_workspace(screen);
    app.save_session()?;
    let reader = ui::workspace_areas(screen, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    let x = reader.right();
    let y = reader.y + 3;
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x + 2, y)?;
    assert_eq!(app.workspace.article_width, 80);
    assert_eq!(app.workspace.library_width, 23);
    assert!(app.workspace.annotation_width < 80);
    Ok(())
}

#[test]
fn docking_and_profiles_preserve_article_and_survive_reopen() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    let article = app.current().context("No article")?.id;
    let original = PanelLayout::capture(&app.workspace);

    command(&mut app, "panel-dock annotations reader below")?;
    let study = PanelLayout::capture(&app.workspace);
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let reader = areas.reader.context("Reader hidden")?;
    let notes = areas.annotations.context("Notes hidden")?;
    assert_eq!(reader.x, notes.x);
    assert_eq!(reader.width, notes.width);
    assert_eq!(reader.bottom(), notes.y);
    command(&mut app, "workspace save My study")?;
    command(&mut app, "layout-undo")?;
    assert_eq!(PanelLayout::capture(&app.workspace), original);
    command(&mut app, "layout-redo")?;
    assert_eq!(PanelLayout::capture(&app.workspace), study);
    command(&mut app, "workspace load Reading")?;
    assert_eq!(app.current().context("Selection lost")?.id, article);
    command(&mut app, "workspace load My study")?;
    app.save_session()?;

    let restored = App::new(
        Store::open(&directory.path().join("library.sqlite3"))?,
        false,
    )?;
    assert_eq!(PanelLayout::capture(&restored.workspace), study);
    assert_eq!(restored.current().context("Selection lost")?.id, article);
    Ok(())
}

#[test]
fn horizontal_seam_resize_cancel_commit_and_history_share_geometry()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "panel-dock annotations reader below")?;
    let original = PanelLayout::capture(&app.workspace);
    let boundary = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .boundaries
        .into_iter()
        .find(|seam| !seam.horizontal)
        .context("No row split")?;
    let x = boundary.second.x + 5;
    let y = boundary.first.bottom() - 1;

    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x, y + 4)?;
    assert_ne!(PanelLayout::capture(&app.workspace), original);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert_eq!(PanelLayout::capture(&app.workspace), original);
    mouse(&mut app, MouseEventKind::Down(MouseButton::Left), x, y)?;
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x, y + 4)?;
    assert_ne!(PanelLayout::capture(&app.workspace), original);
    command(&mut app, "layout-undo")?;
    assert_eq!(PanelLayout::capture(&app.workspace), original);
    Ok(())
}

#[test]
fn malformed_topology_is_rejected_and_collapsed_siblings_fill_their_space()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut workspace = store.workspace()?;
    workspace.layout = Some(PaneLayout::Pane(Pane::Reader));
    assert!(store.save_workspace(&workspace).is_err());
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "panel-dock annotations reader below")?;
    command(&mut app, "panel annotations")?;
    let reader = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    assert_eq!(reader.height, SCREEN.height - 4);
    let tree = PaneLayout::columns(&Pane::ALL);
    assert!(tree.dock(Pane::Reader, Pane::Reader, Dock::Below).is_none());
    for edge in [Dock::Left, Dock::Right, Dock::Above, Dock::Below] {
        assert!(
            tree.dock(Pane::Annotations, Pane::Reader, edge)
                .context("Dock failed")?
                .valid()
        );
    }
    Ok(())
}

#[test]
fn keyboard_prefix_displays_help_and_uses_the_same_split_contract() -> Result<()>
{
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    app.focus = Pane::Reader;
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    assert!(app.layout_prefix_pending());
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE))?;
    assert!(!app.layout_prefix_pending());
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    assert!(areas.boundaries.iter().any(|seam| !seam.horizontal));
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))?;
    assert!(app.workspace.layout.is_none());
    Ok(())
}

#[test]
fn journal_failure_rolls_back_active_preferences_and_history_together()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let original = store.workspace()?;
    store.save_workspace(&original)?;
    let mut next = original.clone();
    next.show_annotations = true;
    let mut history = store.layouts(&original)?;
    history.observe(PanelLayout::capture(&next));
    let injector =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    injector.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_layout_history.sql"
    ))?;

    assert!(store.save_layouts(&next, &history).is_err());

    assert_eq!(
        PanelLayout::capture(&store.workspace()?),
        PanelLayout::capture(&original)
    );
    assert!(store.layouts(&original)?.past.is_empty());
    Ok(())
}

#[test]
fn sub_threshold_grip_motion_does_not_reorder_or_write_history() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .library
        .context("Library hidden")?;
    let grip = ui::pane_move_rect(area).context("Grip hidden")?;
    let original = app.workspace.panel_order;
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        grip.x,
        grip.y,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        grip.x + 1,
        grip.y,
    )?;
    assert!(app.floating_pane().is_none());
    assert_eq!(app.panel_drag().context("Capture missing")?.target, None);
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        grip.x + 1,
        grip.y,
    )?;
    assert_eq!(app.workspace.panel_order, original);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(store.layouts(&store.workspace()?)?.past.is_empty());
    Ok(())
}

#[test]
fn oversized_readers_use_bounded_cards_with_their_complete_outline()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "fullscreen")?;
    app.resize_workspace(SCREEN);
    let reader = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    let grip = ui::pane_move_rect(reader).context("Grip hidden")?;
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        grip.x,
        grip.y,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        grip.x - 8,
        5,
    )?;
    let mut terminal = ratatui::Terminal::new(
        ratatui::backend::TestBackend::new(SCREEN.width, SCREEN.height),
    )?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    let card = app.floating_pane().context("Compact card missing")?;
    assert!(card.area.width <= 38 && card.area.height <= 9);
    for (x, y, symbol) in [
        (card.area.x, card.area.y, "╭"),
        (card.area.right() - 1, card.area.bottom() - 1, "╯"),
    ] {
        assert_eq!(terminal.backend().buffer()[(x, y)].symbol(), symbol);
    }
    Ok(())
}

#[test]
fn mouse_docking_at_a_content_edge_matches_the_keyboard_command() -> Result<()>
{
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "annotations")?;
    let original = PanelLayout::capture(&app.workspace);
    command(&mut app, "panel-dock annotations reader below")?;
    let expected = PanelLayout::capture(&app.workspace);
    command(&mut app, "layout-undo")?;
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let grip = ui::pane_move_rect(areas.annotations.context("Notes hidden")?)
        .context("Grip hidden")?;
    let reader = areas.reader.context("Reader hidden")?;
    let x = reader.x + reader.width / 2;
    let y = reader.bottom() - 2;

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        grip.x,
        grip.y,
    )?;
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), x, y)?;
    assert_eq!(
        app.panel_drag().context("Preview missing")?.edge,
        Some(Dock::Below)
    );
    assert_eq!(PanelLayout::capture(&app.workspace), original);
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), x, y)?;

    assert_eq!(PanelLayout::capture(&app.workspace), expected);
    Ok(())
}

#[test]
fn fullscreen_profiles_restore_reader_focus_instead_of_a_hidden_sidebar()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "fullscreen")?;
    command(&mut app, "workspace save Fullscreen")?;
    command(&mut app, "fullscreen")?;
    command(&mut app, "panel-focus library")?;

    command(&mut app, "workspace load Fullscreen")?;

    assert!(app.workspace.fullscreen);
    assert_eq!(app.focus, Pane::Reader);
    assert_eq!(
        ui::workspace_areas(SCREEN, app.focus, &app.workspace).reader,
        Some(SCREEN)
    );
    command(&mut app, "layout-undo")?;
    assert!(!app.workspace.fullscreen);
    command(&mut app, "layout-redo")?;
    assert!(app.workspace.fullscreen);
    assert_eq!(app.focus, Pane::Reader);
    Ok(())
}

#[test]
fn keyboard_resize_uses_a_visible_seam_when_the_nearest_sibling_is_collapsed()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "workspace load Study")?;
    command(&mut app, "panel annotations")?;
    command(&mut app, "panel-focus reader")?;
    let before = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;

    command(&mut app, "panel-resize +10")?;

    let after = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    assert!(
        after.width > before.width,
        "Resize only changed a hidden split"
    );
    assert!(!app.workspace.show_annotations);
    command(&mut app, "layout-undo")?;
    assert_eq!(
        ui::workspace_areas(SCREEN, app.focus, &app.workspace).reader,
        Some(before)
    );
    Ok(())
}
