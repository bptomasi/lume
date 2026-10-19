//! Visible sidebar rows and mouse hits share section boundaries at every size.

mod common;

use anyhow::{Context, Result};
use lume::{
    app::{App, Focus, NavItem},
    model::View,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

#[test]
fn section_headers_are_not_selectable_and_item_rows_keep_their_identity()
-> Result<()> {
    let (_directory, mut app) = application()?;
    for (width, height) in [(40, 18), (80, 28), (144, 44)] {
        let screen = Rect::new(0, 0, width, height);
        app.resize_workspace(screen);
        let library = ui::workspace_areas(screen, app.focus, &app.workspace)
            .library
            .context("Library pane")?;
        let first = library.y + 1;
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        for (row, expected) in
            [None, None, Some(0), Some(1), None, None, Some(2)]
                .into_iter()
                .enumerate()
        {
            assert_eq!(
                ui::navigation_at(&app, library, first + row as u16),
                expected
            );
        }
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        for title in ["LIBRARY", "First", "Second", "FEEDS", "Third"] {
            assert!(
                text.contains(title),
                "Missing {title} at {width}x{height}"
            );
        }
    }
    Ok(())
}

fn application() -> Result<(tempfile::TempDir, App)> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Library;
    app.workspace.show_articles = false;
    app.workspace.show_reader = false;
    app.workspace.show_annotations = false;
    app.navigation = [
        ("First", "LIBRARY"),
        ("Second", "LIBRARY"),
        ("Third", "FEEDS"),
    ]
    .into_iter()
    .enumerate()
    .map(|(index, (title, group))| NavItem {
        title: title.into(),
        group,
        view: View::Feed(index as i64),
        count: 42,
    })
    .collect();
    app.nav_selected = 0;
    Ok((_directory, app))
}
