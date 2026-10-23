//! Full-list reference rows qualify bounded windows at every selection and size.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::NavItem, model::View, store::Store};

use super::{App, Row, window, window_start};

#[test]
fn stable_windows_match_full_projection_across_repeated_groups() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.navigation = (0..80)
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
        .collect();
    assert_windows(&mut app);
    Ok(())
}

#[test]
fn filtered_windows_keep_original_navigation_identities() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.focus = crate::app::Focus::Library;
    app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))?;
    app.paste("destination")?;
    assert_windows(&mut app);
    Ok(())
}

#[test]
fn selectable_section_titles_share_window_geometry_in_both_states() -> Result<()>
{
    let (_directory, mut app) = application()?;
    assert_windows(&mut app);
    app.focus = crate::app::Focus::Library;
    app.set_view(View::Group("YOUR FOLDERS".into()))?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_windows(&mut app);
    Ok(())
}

fn assert_windows(app: &mut App) {
    let indices = app.navigation_indices().collect::<Vec<_>>();
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
