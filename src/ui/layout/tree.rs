//! Recursive geometry also emits the exact boundaries used by mouse resizing.

use ratatui::layout::{Constraint, Layout, Rect};

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

fn expanded(tree: &PaneLayout, workspace: &Workspace) -> bool {
    match tree {
        PaneLayout::Pane(pane) => workspace.pane_visible(*pane),
        PaneLayout::Split { first, second, .. } => {
            expanded(first, workspace) || expanded(second, workspace)
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
            percent,
        } => {
            let both =
                expanded(first, workspace) && expanded(second, workspace);
            let enough = if *horizontal {
                area.width >= 40
            } else {
                area.height >= 20
            };
            if both && enough {
                divide(tree, area, workspace, focus, path, areas, *percent);
                return;
            }
            let use_first = expanded(first, workspace)
                && (!expanded(second, workspace) || !contains(second, focus));
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
    percent: u16,
) {
    let PaneLayout::Split {
        first,
        second,
        horizontal,
        ..
    } = tree
    else {
        return;
    };
    let extent = if *horizontal { area.width } else { area.height };
    let minimum = if *horizontal { 20 } else { 10 };
    let first_size = ((u32::from(extent) * u32::from(percent) / 100) as u16)
        .clamp(minimum, extent.saturating_sub(minimum));
    let constraints = [Constraint::Length(first_size), Constraint::Fill(1)];
    let rectangles = if *horizontal {
        Layout::horizontal(constraints).split(area)
    } else {
        Layout::vertical(constraints).split(area)
    };
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
