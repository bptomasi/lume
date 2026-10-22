//! Restoring the standard arrangement preserves reading data and preferences.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::App,
    model::{Pane, PanelLayout, Workspace},
    store::Store,
    ui,
};
use ratatui::layout::Rect;

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

#[test]
fn reset_restores_columns_and_widths_and_can_be_undone_after_reopening()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 160, 50));
    app.workspace.theme = "nord".into();
    app.workspace.justify = true;
    app.workspace.reading_width = 90;
    app.workspace.panel_order.swap(0, 1);
    command(&mut app, "panel-dock library reader below")?;
    command(&mut app, "sidebar-width 48")?;
    command(&mut app, "fullscreen")?;
    let before = PanelLayout::capture(&app.workspace);
    let article = app.current().context("No article")?.id;
    let source = app.body.clone();
    command(&mut app, "layout-reset")?;
    assert_standard(&app);
    assert_eq!(app.current().context("Lost article")?.id, article);
    assert_eq!(app.body, source);
    assert_eq!(app.workspace.reading_width, 90);
    assert_eq!(app.workspace.theme, "nord");
    assert!(app.workspace.justify);
    app.end_session()?;
    let mut restored = App::new(
        Store::open(&directory.path().join("library.sqlite3"))?,
        false,
    )?;
    assert_standard(&restored);
    command(&mut restored, "layout-undo")?;
    assert_eq!(PanelLayout::capture(&restored.workspace), before);
    command(&mut restored, "layout-redo")?;
    assert_standard(&restored);
    Ok(())
}

fn assert_standard(app: &App) {
    assert!(app.workspace.layout.is_none());
    assert_eq!(app.workspace.panel_order, Pane::ALL);
    assert!(app.workspace.show_library && app.workspace.show_articles);
    assert!(app.workspace.show_reader && !app.workspace.show_annotations);
    assert!(!app.workspace.fullscreen && !app.workspace.zen);
    assert_eq!(app.workspace.library_width, 23);
    assert_eq!(app.workspace.article_width, 36);
    assert_eq!(app.workspace.annotation_width, 32);
}

#[test]
fn keyboard_reset_is_idempotent_and_discoverable_in_commands() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "workspace load Study")?;
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    app.key(KeyEvent::new(KeyCode::Char('='), KeyModifiers::NONE))?;
    assert_standard(&app);
    command(&mut app, "layout-reset")?;
    command(&mut app, "layout-undo")?;
    assert!(
        app.workspace.layout.is_some(),
        "Repeated reset polluted undo"
    );
    assert!(
        lume::app::command_suggestions("layout-reset")
            .iter()
            .any(|command| command.name == "layout-reset")
    );
    Ok(())
}

#[test]
fn built_in_profiles_start_with_canonical_order_and_proportions() -> Result<()>
{
    let mut workspace = Workspace::default();
    workspace.panel_order.swap(0, 2);
    workspace.library_width = 48;
    workspace.article_width = 80;
    let history = lume::model::LayoutHistory::new(&workspace);
    for name in ["Reading", "Study", "Browse"] {
        let profile = history.profiles.get(name).context("Missing profile")?;
        assert_eq!(profile.order, Pane::ALL, "{name}");
        assert_eq!(profile.widths, [23, 36, 32], "{name}");
    }
    assert_eq!(history.current, PanelLayout::capture(&workspace));
    Ok(())
}

#[test]
fn standard_layout_respects_responsive_focus_in_small_terminals() -> Result<()>
{
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    for width in [48, 80, 119, 120, 160, 240] {
        for focus in [Pane::Library, Pane::Articles, Pane::Reader] {
            app.focus = focus;
            let screen = Rect::new(7, 3, width, 30);
            app.resize_workspace(screen);
            command(&mut app, "layout-reset")?;
            let areas = ui::workspace_areas(screen, app.focus, &app.workspace);
            assert!(areas.get(focus).is_some(), "{width}: {focus:?}");
            if width >= 120 {
                assert_eq!(areas.library.context("No library")?.width, 23);
                assert_eq!(areas.articles.context("No articles")?.width, 36);
            }
        }
    }
    Ok(())
}

#[test]
fn dragging_a_nested_divider_cannot_hide_articles_when_the_terminal_fits()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let screen = Rect::new(0, 0, 160, 50);
    app.resize_workspace(screen);
    command(&mut app, "panel-dock annotations reader below")?;
    app.focus = Pane::Reader;
    let areas = ui::workspace_areas(screen, app.focus, &app.workspace);
    let boundary = areas.boundaries.first().context("Missing root divider")?;
    let row = boundary.first.y + 5;
    for (kind, column) in [
        (MouseEventKind::Down(MouseButton::Left), boundary.second.x),
        (MouseEventKind::Drag(MouseButton::Left), 155),
        (MouseEventKind::Up(MouseButton::Left), 155),
    ] {
        app.mouse(MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        })?;
        let areas = ui::workspace_areas(screen, app.focus, &app.workspace);
        let articles = areas.articles.context("Resize hid Articles")?;
        assert!(articles.width >= 24);
        assert!(areas.reader.is_some());
    }
    Ok(())
}

#[test]
fn notebook_reset_preserves_the_separate_reading_arrangement() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 160, 50));
    command(&mut app, "panel-dock library reader below")?;
    let reading = PanelLayout::capture(&app.workspace);
    command(&mut app, "notebook")?;
    common::settle_notes(&mut app)?;
    let notebook = PanelLayout::capture(&app.workspace);
    command(&mut app, "layout-reset")?;
    assert_standard(&app);
    command(&mut app, "layout-undo")?;
    assert_eq!(PanelLayout::capture(&app.workspace), notebook);
    command(&mut app, "layout-redo")?;
    assert_standard(&app);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.notebook.is_none());
    assert_eq!(PanelLayout::capture(&app.workspace), reading);
    command(&mut app, "notebook")?;
    common::settle_notes(&mut app)?;
    assert_standard(&app);
    app.end_session()?;
    Ok(())
}

#[test]
fn narrow_headers_keep_a_clean_separator_between_pane_controls() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "sidebar-width 16")?;
    let screen = Rect::new(0, 0, 160, 40);
    app.resize_workspace(screen);
    let library = ui::workspace_areas(screen, app.focus, &app.workspace)
        .library
        .context("Library missing")?;
    let grip = ui::pane_move_rect(library).context("Move grip missing")?;
    let mut terminal = ratatui::Terminal::new(
        ratatui::backend::TestBackend::new(screen.width, screen.height),
    )?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(grip.right(), grip.y)].symbol(), "─");
    assert_eq!(buffer[(library.right() - 1, library.y)].symbol(), "╮");
    Ok(())
}
