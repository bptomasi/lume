//! Full-list reference rows qualify bounded windows at every selection and size.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::NavItem, model::View, store::Store};

use super::{App, NavigationView, Row, window, window_start};

#[test]
fn stable_windows_match_full_projection_across_repeated_groups() -> Result<()> {
    let (_directory, app) = application()?;
    let items = (0..80)
        .map(|index| NavItem {
            view: View::Feed(index),
            title: index.to_string(),
            group: match index % 7 {
                0..=2 => "FEEDS",
                3..=4 => "PINNED",
                _ => "SMART LISTS",
            },
            count: 0,
        })
        .collect::<Vec<_>>();
    let mut navigation = app.navigation();
    navigation.navigation = &items;
    assert_windows(navigation);
    Ok(())
}

#[test]
fn filtered_windows_keep_original_navigation_identities() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.focus = crate::app::Focus::Library;
    app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))?;
    app.paste("destination")?;
    assert_windows(app.navigation());
    Ok(())
}

#[test]
fn selectable_section_titles_share_window_geometry_in_both_states() -> Result<()>
{
    let (_directory, mut app) = application()?;
    assert_windows(app.navigation());
    app.focus = crate::app::Focus::Library;
    app.set_view(View::Group("YOUR FOLDERS".into()))?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_windows(app.navigation());
    Ok(())
}

fn assert_windows(mut app: NavigationView<'_>) {
    let indices = app.indices().collect::<Vec<_>>();
    let mut expected = Vec::new();
    let mut group = "";
    for index in &indices {
        let item = &app.navigation[*index];
        if item.group != group {
            group = item.group;
            expected.push(Row::Blank);
            if !matches!(item.view, View::Group(_)) {
                expected.push(Row::Heading(group));
            }
        }
        expected.push(Row::Item(*index));
    }
    for height in [0, 1, 2, 3, 8, 19, 44] {
        app.navigation_first = 0;
        app.navigation_skip = 0;
        let mut start = 0;
        for selected in indices.iter().chain(indices.iter().rev()) {
            app.nav_selected = *selected;
            let position = expected
                .iter()
                .position(|row| *row == Row::Item(*selected))
                .unwrap_or_default();
            if position < start {
                start = position;
            } else if position >= start + usize::from(height) {
                start = position
                    .saturating_sub(usize::from(height).saturating_sub(1));
            }
            let reference = expected[start..]
                .iter()
                .copied()
                .take(usize::from(height))
                .collect::<Vec<_>>();
            assert_eq!(window(app, height).collect::<Vec<_>>(), reference);
            (app.navigation_first, app.navigation_skip) =
                window_start(app, height);
        }
    }
}

fn application() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    for index in 0..60 {
        store.create_collection(&format!("Destination {index}"))?;
    }
    Ok((directory, App::new(store, false)?))
}

#[test]
fn narrow_section_controls_prioritize_the_title_over_the_aggregate_count() {
    use unicode_width::UnicodeWidthStr;
    let item = NavItem {
        view: View::Group("SMART LISTS".into()),
        title: "▾ SMART LISTS".into(),
        group: "SMART LISTS",
        count: 123_456,
    };
    for width in [14, 20, 28] {
        let text = super::section_text(&item, width);
        assert!(text.contains("SMART LISTS"));
        assert!(text.width() <= usize::from(width));
    }
}

fn three_rows() -> Vec<NavItem> {
    [
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
    .collect()
}

#[test]
fn static_labels_are_not_selectable_and_item_rows_keep_their_identity()
-> Result<()> {
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    let (_directory, app) = application()?;
    let items = three_rows();
    let mut navigation = app.navigation();
    navigation.navigation = &items;
    navigation.nav_selected = 0;
    for (width, height) in [(40, 18), (80, 28), (144, 44)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| {
            super::draw(frame, navigation, Rect::new(0, 0, width, height))
        })?;
        for (row, expected) in
            [None, None, Some(0), Some(1), None, None, Some(2)]
                .into_iter()
                .enumerate()
        {
            let actual = match window(navigation, height).nth(row) {
                Some(Row::Item(index)) => Some(index),
                _ => None,
            };
            assert_eq!(actual, expected);
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

#[test]
fn wide_titles_keep_large_counts_inside_the_sidebar() -> Result<()> {
    use ratatui::{
        Terminal,
        backend::TestBackend,
        layout::Rect,
        widgets::{Block, Borders},
    };
    let (_directory, app) = application()?;
    let mut items = three_rows();
    items[0].title = "界面 🦀 e\u{301} ".repeat(20);
    items[0].count = 123_456;
    let mut navigation = app.navigation();
    navigation.navigation = &items;
    navigation.nav_selected = 0;
    let area = Rect::new(0, 0, 40, 18);
    let mut terminal = Terminal::new(TestBackend::new(40, 18))?;
    terminal.draw(|frame| {
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(area);
        frame.render_widget(block, area);
        super::draw(frame, navigation, inner);
    })?;
    let row = area.y + 3;
    let count = (area.right() - 8..area.right() - 1)
        .map(|column| terminal.backend().buffer()[(column, row)].symbol())
        .collect::<String>();
    assert_eq!(count, "123456 ");
    assert_eq!(
        terminal.backend().buffer()[(area.right() - 1, row)].symbol(),
        "│"
    );
    Ok(())
}
