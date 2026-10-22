//! Recursive geometry also emits the exact boundaries used by mouse resizing.

use ratatui::layout::{Rect, Size};

use crate::model::{Pane, PaneLayout, Workspace};

use super::{Areas, SplitBoundary};

pub(super) fn arrange(
    body: Rect,
    tree: &PaneLayout,
    workspace: &Workspace,
    focus: Pane,
) -> Areas {
    let mut areas = Areas::default();
    place(tree, body, workspace, focus, &mut Vec::new(), &mut areas);
    areas
}

fn minimum(tree: &PaneLayout, workspace: &Workspace) -> Size {
    match tree {
        PaneLayout::Pane(pane) if workspace.pane_visible(*pane) => {
            Size::new(pane.minimum_width(), 10)
        }
        PaneLayout::Pane(_) => Size::default(),
        PaneLayout::Split {
            first,
            second,
            horizontal,
            ..
        } => {
            let first = minimum(first, workspace);
            let second = minimum(second, workspace);
            if *horizontal {
                Size::new(
                    first.width + second.width,
                    first.height.max(second.height),
                )
            } else {
                Size::new(
                    first.width.max(second.width),
                    first.height + second.height,
                )
            }
        }
    }
}

fn contains(tree: &PaneLayout, pane: Pane) -> bool {
    match tree {
        PaneLayout::Pane(id) => *id == pane,
        PaneLayout::Split { first, second, .. } => {
            contains(first, pane) || contains(second, pane)
        }
    }
}

fn place(
    tree: &PaneLayout,
    area: Rect,
    workspace: &Workspace,
    focus: Pane,
    path: &mut Vec<bool>,
    areas: &mut Areas,
) {
    match tree {
        PaneLayout::Pane(pane) if workspace.pane_visible(*pane) => {
            areas.insert(*pane, area);
        }
        PaneLayout::Split {
            first,
            second,
            horizontal,
            ..
        } => {
            let first_size = minimum(first, workspace);
            let second_size = minimum(second, workspace);
            let both = first_size.width > 0 && second_size.width > 0;
            let (extent, limits) = if *horizontal {
                (area.width, [first_size.width, second_size.width])
            } else {
                (area.height, [first_size.height, second_size.height])
            };
            let enough = extent >= limits.iter().sum();
            if both && enough {
                divide(tree, area, workspace, focus, path, areas, limits);
                return;
            }
            let use_first = first_size.width > 0
                && (second_size.width == 0 || !contains(second, focus));
            let child = if use_first { first } else { second };
            path.push(!use_first);
            place(child, area, workspace, focus, path, areas);
            path.pop();
        }
        _ => {}
    }
}

fn divide(
    tree: &PaneLayout,
    area: Rect,
    workspace: &Workspace,
    focus: Pane,
    path: &mut Vec<bool>,
    areas: &mut Areas,
    limits: [u16; 2],
) {
    let PaneLayout::Split {
        first,
        second,
        horizontal,
        percent,
        ..
    } = tree
    else {
        return;
    };
    let rectangles = split_regions(area, *horizontal, *percent, limits);
    areas.boundaries.push(SplitBoundary {
        first: rectangles[0],
        second: rectangles[1],
        horizontal: *horizontal,
        path: path.clone(),
    });
    for (trailing, child, rectangle) in
        [(false, first, rectangles[0]), (true, second, rectangles[1])]
    {
        path.push(trailing);
        place(child, rectangle, workspace, focus, path, areas);
        path.pop();
    }
}

fn split_regions(
    area: Rect,
    horizontal: bool,
    percent: u16,
    limits: [u16; 2],
) -> [Rect; 2] {
    let extent = if horizontal { area.width } else { area.height };
    let first_size = ((u32::from(extent) * u32::from(percent) / 100) as u16)
        .clamp(limits[0], extent.saturating_sub(limits[1]));
    if horizontal {
        [
            Rect::new(area.x, area.y, first_size, area.height),
            Rect::new(
                area.x + first_size,
                area.y,
                area.width - first_size,
                area.height,
            ),
        ]
    } else {
        [
            Rect::new(area.x, area.y, area.width, first_size),
            Rect::new(
                area.x,
                area.y + first_size,
                area.width,
                area.height - first_size,
            ),
        ]
    }
}
