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
    /// Whether every pane is collapsed or suppressed.
    pub fn is_empty(&self) -> bool {
        Pane::ALL.into_iter().all(|pane| self.get(pane).is_none())
    }

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
    let body = Rect::new(
        area.x,
        area.y + area.height.min(2),
        area.width,
        area.height.saturating_sub(4),
    );
    if workspace.zen && workspace.show_reader {
        return single(body, Pane::Reader);
    }
    if let Some(tree) = &workspace.layout {
        return tree::arrange(body, tree, workspace, focus);
    }
    let panes = responsive_panes(body.width, focus, workspace);
    let widths = pane_widths(body.width, &panes, workspace);
    let mut areas = Areas::default();
    let mut column = body.x;
    for (pane, width) in panes.into_iter().zip(widths) {
        areas.insert(pane, Rect::new(column, body.y, width, body.height));
        column += width;
    }
    areas
}

fn single(area: Rect, pane: Pane) -> Areas {
    let mut areas = Areas::default();
    areas.insert(pane, area);
    areas
}

fn pane_widths(width: u16, panes: &[Pane], workspace: &Workspace) -> [u16; 4] {
    let flexible = if panes.contains(&Pane::Reader) {
        Some(Pane::Reader)
    } else {
        panes.last().copied()
    };
    let mut remaining =
        width.saturating_sub(flexible.map_or(0, Pane::minimum_width));
    let mut minimums = panes
        .iter()
        .filter(|pane| Some(**pane) != flexible)
        .map(|pane| pane.minimum_width())
        .sum::<u16>();
    let mut widths = [0; 4];
    for (index, pane) in panes.iter().enumerate() {
        if Some(*pane) == flexible {
            continue;
        }
        let minimum = pane.minimum_width();
        minimums = minimums.saturating_sub(minimum);
        let available = remaining.saturating_sub(minimums);
        let preferred = workspace.pane_width(*pane).unwrap_or(20);
        widths[index] = preferred.min(available);
        remaining = remaining.saturating_sub(widths[index]);
    }
    if let Some(index) = panes.iter().position(|pane| Some(*pane) == flexible) {
        widths[index] = width.saturating_sub(widths.iter().sum());
    }
    widths
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
        area.y.saturating_add(area.height.min(6)),
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
    /// Requested share before minimum pane sizes constrain the rectangles.
    pub percent: u16,
}

impl SplitBoundary {
    /// Adjust from the visible seam when minimum dimensions constrain it.
    /// Otherwise preserve percentage changes smaller than a terminal cell.
    pub fn adjusted_percent(&self, delta: i32) -> u16 {
        if delta == 0 {
            return self.percent;
        }
        let (first, extent) = self.extents();
        let requested = extent * u32::from(self.percent) / 100;
        let base = if requested == first {
            self.percent
        } else {
            (first * 100).div_ceil(extent.max(1)) as u16
        };
        i32::from(base).saturating_add(delta).clamp(15, 85) as u16
    }

    /// A stationary pointer retains the saved ratio exactly, including clamps.
    pub fn dragged_percent(&self, delta: i32) -> u16 {
        if delta == 0 {
            return self.percent;
        }
        let (_, extent) = self.extents();
        let extent = i32::try_from(extent.max(1)).unwrap_or(1);
        let rounded = (delta * 100 + delta.signum() * (extent / 2)) / extent;
        self.adjusted_percent(rounded)
    }

    fn extents(&self) -> (u32, u32) {
        if self.horizontal {
            (
                u32::from(self.first.width),
                u32::from(self.first.width) + u32::from(self.second.width),
            )
        } else {
            (
                u32::from(self.first.height),
                u32::from(self.first.height) + u32::from(self.second.height),
            )
        }
    }
}
