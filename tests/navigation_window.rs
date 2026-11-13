//! Visible sidebar rows and mouse hits share section boundaries at every size.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use lume::{
    app::{App, Focus},
    model::View,
    ui,
};
use ratatui::layout::Rect;

#[test]
fn clicking_visible_destinations_does_not_recenter_or_change_pointer_targets()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let screen = Rect::new(0, 0, 80, 24);
    app.resize_workspace(screen);
    let destination = app
        .navigation()
        .navigation
        .iter()
        .filter(|item| matches!(item.view, View::Feed(_)))
        .nth(30)
        .context("Middle feed")?
        .view
        .clone();
    app.set_view(destination)?;
    app.resize_workspace(screen);
    let area = ui::workspace_areas(screen, app.focus(), app.workspace())
        .library
        .context("Library pane")?;
    let targets = |app: &App| {
        (area.y + 1..area.bottom() - 1)
            .map(|row| ui::navigation_at(app.navigation_hits(), area, row))
            .collect::<Vec<_>>()
    };
    let before = targets(&app);
    for local in [3, 15, 8, 8] {
        let expected = before[local].context("Expected selectable row")?;
        app.mouse(MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: area.x + 3,
            row: area.y + 1 + local as u16,
            modifiers: KeyModifiers::NONE,
        })?;
        assert_eq!(app.navigation().nav_selected, expected);
        assert_eq!(targets(&app), before, "Click moved the viewport");
    }
    Ok(())
}

fn application() -> Result<(tempfile::TempDir, App)> {
    let (directory, store, _feed) = common::seeded_store()?;
    for index in 0..80 {
        store.subscribe(&format!("https://feed{index}.example/rss"))?;
    }
    let mut app = App::new(store, false)?;
    app.activate_pane(Focus::Library);
    common::fixture_pane(&mut app, lume::model::Pane::Articles, false)?;
    common::fixture_pane(&mut app, lume::model::Pane::Reader, false)?;
    common::fixture_pane(&mut app, lume::model::Pane::Annotations, false)?;
    app.set_view(View::All)?;
    Ok((directory, app))
}
