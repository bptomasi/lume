//! One geometry owner for rendering, pane controls and pointer hit testing.

mod tree;

use ratatui::layout::{Constraint, Layout, Rect};

use crate::model::{Pane, Workspace};

/// Responsive pane rectangles shared with the runtime and pointer controller.
#[derive(Default)]
pub struct Areas {
    /// Custom split seams used for pointer resizing.
    pub boundaries: Vec<SplitBoundary>,
    /// Subscriptions, smart lists and folders.
    pub library: Option<Rect>,
    /// Article headlines.
    pub articles: Option<Rect>,
    /// Article reader.
    pub reader: Option<Rect>,
    /// Position-ordered article annotations.
    pub annotations: Option<Rect>,
}

impl Areas {
    /// Rectangle of a visible pane, or none when collapsed/suppressed.
    pub fn get(&self, pane: Pane) -> Option<Rect> {
        match pane {
            Pane::Library => self.library,
            Pane::Articles => self.articles,
            Pane::Reader => self.reader,
            Pane::Annotations => self.annotations,
        }
    }

    /// Visible panes in column, then row order.
    pub fn ordered(&self) -> Vec<(Pane, Rect)> {
        let mut panes = Pane::ALL
            .into_iter()
            .filter_map(|pane| self.get(pane).map(|area| (pane, area)))
            .collect::<Vec<_>>();
        panes.sort_by_key(|(_, area)| (area.x, area.y));
        panes
    }

    fn insert(&mut self, pane: Pane, area: Rect) {
        match pane {
            Pane::Library => self.library = Some(area),
            Pane::Articles => self.articles = Some(area),
            Pane::Reader => self.reader = Some(area),
            Pane::Annotations => self.annotations = Some(area),
        }
    }
}

/// Compute default responsive geometry for previews and integrations.
pub fn areas(area: Rect, focus: Pane) -> Areas {
    workspace_areas(area, focus, &Workspace::default())
}

/// Honor visibility/order while keeping the focused expanded pane accessible.
pub fn workspace_areas(
    area: Rect,
    focus: Pane,
    workspace: &Workspace,
) -> Areas {
    if workspace.fullscreen {
        return single(area, Pane::Reader);
    }
    let body = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area)[1];
    if workspace.zen && workspace.show_reader {
        return single(body, Pane::Reader);
    }
    if let Some(tree) = &workspace.layout {
        return tree::arrange(body, tree, workspace, focus);
    }
    let panes = responsive_panes(body.width, focus, workspace);
    let constraints = pane_constraints(body.width, &panes, workspace);
    let rectangles = Layout::horizontal(constraints).split(body);
    let mut areas = Areas::default();
    for (pane, area) in panes.into_iter().zip(rectangles.iter()) {
        areas.insert(pane, *area);
    }
    areas
}

fn single(area: Rect, pane: Pane) -> Areas {
    let mut areas = Areas::default();
    areas.insert(pane, area);
    areas
}

fn pane_constraints(
    width: u16,
    panes: &[Pane],
    workspace: &Workspace,
) -> Vec<Constraint> {
    let flexible = if panes.contains(&Pane::Reader) {
        Some(Pane::Reader)
    } else {
        panes.last().copied()
    };
    let mut remaining = width.saturating_sub(20);
    let mut minimums = panes
        .iter()
        .filter(|pane| Some(**pane) != flexible)
        .map(|pane| if *pane == Pane::Library { 16 } else { 24 })
        .sum::<u16>();
    panes
        .iter()
        .map(|pane| {
            if Some(*pane) == flexible {
                return Constraint::Fill(1);
            }
            let minimum = if *pane == Pane::Library { 16 } else { 24 };
            minimums = minimums.saturating_sub(minimum);
            let available = remaining.saturating_sub(minimums);
            let preferred = workspace.pane_width(*pane).unwrap_or(20);
            let size = preferred.min(available);
            remaining = remaining.saturating_sub(size);
            Constraint::Length(size)
        })
        .collect()
}

fn responsive_panes(
    width: u16,
    focus: Pane,
    workspace: &Workspace,
) -> Vec<Pane> {
    let expanded = workspace
        .panel_order
        .into_iter()
        .filter(|pane| workspace.pane_visible(*pane))
        .collect::<Vec<_>>();
    let active = if expanded.contains(&focus) {
        focus
    } else {
        expanded.first().copied().unwrap_or(Pane::Reader)
    };
    if width < 80 {
        return expanded
            .contains(&active)
            .then_some(active)
            .into_iter()
            .collect();
    }
    if width < 120 {
        let secondary = if active == Pane::Library {
            expanded.iter().copied().find(|pane| *pane != Pane::Library)
        } else {
            Some(active)
        };
        return expanded
            .into_iter()
            .filter(|pane| *pane == Pane::Library || Some(*pane) == secondary)
            .collect();
    }
    expanded
}

/// Centered text geometry, excluding the frame and article metadata.
pub fn reader_content(area: Rect, width: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(4));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y.saturating_add(6),
        width,
        area.height.saturating_sub(8),
    )
}

/// Center prose in both layouts, clamping preferences to the available space.
pub fn workspace_reader_content(area: Rect, workspace: &Workspace) -> Rect {
    reader_content(
        area,
        if workspace.fullscreen {
            workspace.fullscreen_width
        } else {
            workspace.reading_width
        },
    )
}

/// Persistent collapse/restore buttons; all remain reachable in a 24-cell TUI.
pub fn pane_buttons(area: Rect, workspace: &Workspace) -> Vec<(Pane, Rect)> {
    let width = (area.width / 4).clamp(1, 19);
    workspace
        .panel_order
        .into_iter()
        .enumerate()
        .map(|(index, pane)| {
            (
                pane,
                Rect::new(area.x + index as u16 * width, area.y + 1, width, 1),
            )
        })
        .collect()
}

/// Visible move grip beside collapse, without overlapping its hit region.
pub fn pane_move_rect(area: Rect) -> Option<Rect> {
    (area.width >= 12 && area.height > 0)
        .then(|| Rect::new(area.right() - 8, area.y, 3, 1))
}

/// Three-cell collapse control on a pane's title row.
pub fn pane_close_rect(area: Rect) -> Option<Rect> {
    (area.width >= 8 && area.height > 0)
        .then(|| Rect::new(area.right() - 4, area.y, 3, 1))
}

/// Choice and preview rectangles shared by selector drawing and mouse input.
pub fn picker_regions(screen: Rect) -> (Rect, Rect) {
    let outer = super::popup::Popup::Picker.area(screen);
    let inner = outer.inner(ratatui::layout::Margin::new(1, 1));
    let body = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    let panes = if body.width >= 80 {
        Layout::horizontal([
            Constraint::Percentage(52),
            Constraint::Percentage(48),
        ])
        .split(body)
    } else {
        Layout::vertical([
            Constraint::Percentage(65),
            Constraint::Percentage(35),
        ])
        .split(body)
    };
    (panes[0], panes[1])
}

/// A rendered split's two rectangles and bounded path in the saved topology.
#[derive(Clone)]
pub struct SplitBoundary {
    /// Leading child rectangle.
    pub first: Rect,
    /// Trailing child rectangle.
    pub second: Rect,
    /// True for column allocation, false for row allocation.
    pub horizontal: bool,
    /// False traverses first, true traverses second.
    pub path: Vec<bool>,
}
