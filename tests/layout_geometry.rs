//! Pane geometry conserves space, focus and hit boundaries across responsive sizes.

use lume::{
    model::{Dock, Pane, PaneLayout, Workspace},
    ui,
};
use ratatui::layout::Rect;

#[test]
fn responsive_splits_partition_the_body_and_retain_every_visible_focus() {
    let columns = PaneLayout::columns(&Pane::ALL);
    let trees = [
        None,
        Some(columns.clone()),
        columns.dock(Pane::Annotations, Pane::Reader, Dock::Below),
        columns.dock(Pane::Library, Pane::Articles, Dock::Above),
    ];
    for tree in trees {
        for mask in 0..16 {
            let mut workspace = Workspace {
                layout: tree.clone(),
                ..Workspace::default()
            };
            for (bit, pane) in Pane::ALL.into_iter().enumerate() {
                workspace.set_pane_visible(pane, mask & (1 << bit) != 0);
            }
            for width in [0, 23, 39, 40, 79, 80, 119, 120, 160, 240] {
                for height in [0, 3, 10, 23, 24, 50] {
                    for focus in Pane::ALL {
                        let screen = Rect::new(7, 3, width, height);
                        let areas =
                            ui::workspace_areas(screen, focus, &workspace);
                        assert_partition(&areas, screen, mask != 0);
                        if workspace.pane_visible(focus) {
                            assert!(
                                areas.get(focus).is_some(),
                                "Lost {focus:?} at {width}x{height}"
                            );
                        }
                    }
                }
            }
        }
    }
}

fn assert_partition(areas: &ui::Areas, screen: Rect, expanded: bool) {
    let mut cells = 0;
    for (index, pane) in Pane::ALL.into_iter().enumerate() {
        let Some(area) = areas.get(pane) else {
            continue;
        };
        assert_eq!(area.intersection(screen), area);
        cells += u32::from(area.width) * u32::from(area.height);
        for neighbor in &Pane::ALL[index + 1..] {
            if let Some(other) = areas.get(*neighbor) {
                assert!(area.intersection(other).is_empty());
            }
        }
    }
    let expected = if expanded {
        u32::from(screen.width) * u32::from(screen.height.saturating_sub(4))
    } else {
        0
    };
    assert_eq!(cells, expected, "Lost body space at {screen:?}");
    for boundary in &areas.boundaries {
        if boundary.horizontal {
            assert_eq!(boundary.first.right(), boundary.second.x);
        } else {
            assert_eq!(boundary.first.bottom(), boundary.second.y);
        }
    }
}

#[test]
fn reader_content_never_escapes_short_panels() {
    for height in 0..12 {
        let area = Rect::new(20, 30, 100, height);
        let content = ui::reader_content(area, 80);
        assert_eq!(content.intersection(area), content);
    }
}

#[test]
fn extreme_split_ratios_keep_articles_and_siblings_above_their_minimums() {
    let columns = PaneLayout::columns(&Pane::ALL);
    for ratio in [15, 85] {
        for tree in [
            Some(columns.clone()),
            columns.dock(Pane::Annotations, Pane::Reader, Dock::Below),
        ] {
            let mut workspace = Workspace {
                layout: tree,
                show_annotations: true,
                ..Workspace::default()
            };
            if let Some(tree) = &mut workspace.layout {
                set_ratios(tree, ratio);
            }
            for width in [120, 160, 240] {
                let areas = ui::workspace_areas(
                    Rect::new(0, 0, width, 50),
                    Pane::Reader,
                    &workspace,
                );
                for (pane, minimum) in [
                    (Pane::Library, 16),
                    (Pane::Articles, 24),
                    (Pane::Reader, 20),
                    (Pane::Annotations, 24),
                ] {
                    let area = areas.get(pane);
                    assert!(
                        area.is_some_and(|area| area.width >= minimum),
                        "{pane:?} vanished or shrank: {width} / {ratio}"
                    );
                }
            }
        }
    }
}

fn set_ratios(tree: &mut PaneLayout, ratio: u16) {
    if let PaneLayout::Split {
        percent,
        first,
        second,
        ..
    } = tree
    {
        *percent = ratio;
        set_ratios(first, ratio);
        set_ratios(second, ratio);
    }
}
