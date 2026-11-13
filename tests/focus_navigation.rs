//! Directional focus follows geometry; cycling remains a separate operation.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    model::{Dock, Pane, PaneLayout, PanelLayout},
    ui,
};
use ratatui::layout::Rect;

const SCREEN: Rect = Rect::new(0, 0, 160, 50);

#[test]
fn library_g_prefix_is_consumed_by_the_next_navigation_key() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.activate_pane(Pane::Library);
    key(&mut app, KeyCode::Char('G'))?;
    key(&mut app, KeyCode::Char('g'))?;
    key(&mut app, KeyCode::Char('k'))?;
    let selected = app.navigation().nav_selected;
    assert!(selected > 0, "Fixture needs a destination below the first");

    key(&mut app, KeyCode::Char('g'))?;
    assert_eq!(app.navigation().nav_selected, selected);
    key(&mut app, KeyCode::Char('g'))?;
    assert_eq!(app.navigation().nav_selected, 0);
    Ok(())
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn layout_key(app: &mut App, code: char) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    key(app, KeyCode::Char(code))
}

#[test]
fn articles_move_left_to_library_despite_a_different_saved_order() -> Result<()>
{
    let (_directory, store, _) = common::seeded_store()?;
    let mut workspace = store.workspace()?;
    workspace.layout = Some(PaneLayout::columns(&Pane::ALL));
    workspace.panel_order = [
        Pane::Reader,
        Pane::Articles,
        Pane::Library,
        Pane::Annotations,
    ];
    store.save_workspace(&workspace)?;
    let mut app = App::new(store, false)?;
    app.activate_pane(Pane::Articles);
    app.resize_workspace(SCREEN);
    let article = app.current().context("No article")?.id;
    let areas = ui::workspace_areas(SCREEN, app.focus(), app.workspace());
    assert_eq!(
        areas.library.context("Library hidden")?.right(),
        areas.articles.context("Articles hidden")?.x
    );

    key(&mut app, KeyCode::Char('h'))?;

    assert_eq!(app.focus(), Pane::Library);
    assert_eq!(app.current().context("Selection lost")?.id, article);
    key(&mut app, KeyCode::Char('l'))?;
    assert_eq!(app.focus(), Pane::Articles);
    key(&mut app, KeyCode::Left)?;
    assert_eq!(app.focus(), Pane::Library);
    Ok(())
}

#[test]
fn directional_keys_stop_at_edges_while_tab_keeps_cycling() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    app.activate_pane(Pane::Library);

    key(&mut app, KeyCode::Char('h'))?;
    assert_eq!(app.focus(), Pane::Library);
    layout_key(&mut app, 'k')?;
    assert_eq!(app.focus(), Pane::Library);
    key(&mut app, KeyCode::BackTab)?;
    assert_eq!(app.focus(), Pane::Reader);
    key(&mut app, KeyCode::Tab)?;
    assert_eq!(app.focus(), Pane::Library);
    Ok(())
}

#[test]
fn split_focus_reaches_neighbors_in_each_direction() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut workspace = store.workspace()?;
    let tree = PaneLayout::columns(&Pane::ALL);
    workspace.layout = Some(
        tree.dock(Pane::Articles, Pane::Reader, Dock::Above)
            .context("Cannot dock")?,
    );
    store.save_workspace(&workspace)?;
    let mut app = App::new(store, false)?;
    app.activate_pane(Pane::Articles);
    app.resize_workspace(SCREEN);

    layout_key(&mut app, 'j')?;
    assert_eq!(app.focus(), Pane::Reader);
    layout_key(&mut app, 'h')?;
    assert_eq!(app.focus(), Pane::Library);
    layout_key(&mut app, 'l')?;
    assert_eq!(app.focus(), Pane::Articles);
    layout_key(&mut app, 'j')?;
    assert_eq!(app.focus(), Pane::Reader);
    layout_key(&mut app, 'k')?;
    assert_eq!(app.focus(), Pane::Articles);
    Ok(())
}

#[test]
fn collapsed_panels_are_skipped_without_changing_the_layout() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    common::fixture_pane(&mut app, lume::model::Pane::Articles, false)?;
    app.resize_workspace(SCREEN);
    app.activate_pane(Pane::Reader);
    let workspace = PanelLayout::capture(app.workspace());

    layout_key(&mut app, 'h')?;

    assert_eq!(app.focus(), Pane::Library);
    assert_eq!(PanelLayout::capture(app.workspace()), workspace);
    key(&mut app, KeyCode::Char('l'))?;
    assert_eq!(app.focus(), Pane::Reader);
    Ok(())
}

#[test]
fn narrow_terminals_reach_responsively_hidden_neighbors() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.activate_pane(Pane::Articles);
    app.resize_workspace(Rect::new(0, 0, 60, 20));

    key(&mut app, KeyCode::Char('h'))?;
    assert_eq!(app.focus(), Pane::Library);
    key(&mut app, KeyCode::Char('l'))?;
    assert_eq!(app.focus(), Pane::Articles);
    layout_key(&mut app, 'l')?;
    assert_eq!(app.focus(), Pane::Reader);
    Ok(())
}

#[test]
fn docking_commands_change_the_direction_without_changing_tab_order()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    let order = app.workspace().panel_order;
    key(&mut app, KeyCode::Char(':'))?;
    app.paste("panel-dock library articles right")?;
    key(&mut app, KeyCode::Enter)?;
    app.activate_pane(Pane::Library);
    let areas = ui::workspace_areas(SCREEN, app.focus(), app.workspace());
    assert_eq!(
        areas.articles.context("Articles hidden")?.right(),
        areas.library.context("Library hidden")?.x
    );
    assert_eq!(app.workspace().panel_order, order);

    key(&mut app, KeyCode::Char('h'))?;
    assert_eq!(app.focus(), Pane::Articles);
    key(&mut app, KeyCode::Right)?;
    assert_eq!(app.focus(), Pane::Library);
    layout_key(&mut app, 'h')?;
    assert_eq!(app.focus(), Pane::Articles);
    Ok(())
}

#[test]
fn small_split_layouts_preserve_vertical_navigation() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut workspace = store.workspace()?;
    workspace.layout = Some(
        PaneLayout::columns(&Pane::ALL)
            .dock(Pane::Articles, Pane::Reader, Dock::Above)
            .context("Cannot dock")?,
    );
    store.save_workspace(&workspace)?;
    let mut app = App::new(store, false)?;
    app.activate_pane(Pane::Articles);
    app.resize_workspace(Rect::new(0, 0, 30, 12));

    layout_key(&mut app, 'j')?;
    assert_eq!(app.focus(), Pane::Reader);
    layout_key(&mut app, 'k')?;
    assert_eq!(app.focus(), Pane::Articles);
    key(&mut app, KeyCode::Char('h'))?;
    assert_eq!(app.focus(), Pane::Library);
    Ok(())
}

#[test]
fn directional_focus_leaves_fullscreen_and_zen() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    for fullscreen in [true, false] {
        app.activate_pane(Pane::Reader);
        common::fixture_toggle(&mut app, "fullscreen", fullscreen)?;
        common::fixture_toggle(&mut app, "zen", !fullscreen)?;

        layout_key(&mut app, 'h')?;

        assert_eq!(app.focus(), Pane::Articles);
        assert!(!app.workspace().fullscreen);
        assert!(!app.workspace().zen);
    }
    Ok(())
}
