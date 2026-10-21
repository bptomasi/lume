//! Full-list reference rows qualify bounded windows at every selection and size.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{app::NavItem, model::View, store::Store};

use super::{App, Row, window};

#[test]
fn centered_windows_match_full_projection_across_repeated_groups() -> Result<()>
{
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

fn assert_windows(app: &mut App) {
    let indices = app.navigation_indices().collect::<Vec<_>>();
    let mut expected = Vec::new();
    let mut group = "";
    for index in &indices {
        let item = &app.navigation[*index];
        if item.group != group {
            group = item.group;
            expected.extend([Row::Blank, Row::Heading(group)]);
        }
        expected.push(Row::Item(*index));
    }
    for selected in indices {
        app.nav_selected = selected;
        let position = expected
            .iter()
            .position(|row| *row == Row::Item(selected))
            .unwrap_or_default();
        for height in [0, 1, 2, 3, 8, 19, 44] {
            let start = position.saturating_sub(usize::from(height) / 2);
            let reference = expected[start..]
                .iter()
                .copied()
                .take(usize::from(height))
                .collect::<Vec<_>>();
            assert_eq!(window(app, height).collect::<Vec<_>>(), reference);
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
