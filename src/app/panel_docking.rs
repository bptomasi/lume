//! Spatial keyboard operations delegate to the same docking contract as mouse.

use anyhow::{Context, Result};
use ratatui::layout::Rect;

use crate::{
    model::{Dock, Pane},
    ui,
};

use super::App;

impl App {
    pub(super) fn split_focused(&mut self, edge: Dock) -> Result<()> {
        let target = self.focus;
        let pane = crate::model::Pane::ALL
            .into_iter()
            .find(|pane| !self.workspace.pane_visible(*pane))
            .or_else(|| {
                self.expanded_panes()
                    .into_iter()
                    .find(|pane| *pane != target)
            })
            .context("Another pane is required")?;
        self.dock_pane(pane, target, edge)?;
        self.focus = pane;
        self.commit_panel_layout()
    }

    pub(super) fn move_spatially(&mut self, edge: Dock) -> Result<()> {
        let target = self
            .spatial_neighbor(edge)
            .or_else(|| {
                self.expanded_panes()
                    .into_iter()
                    .find(|pane| *pane != self.focus)
            })
            .context("Expand another pane first")?;
        self.dock_pane(self.focus, target, edge)?;
        self.commit_panel_layout()
    }

    pub(super) fn resize_visible_split(&mut self, delta: i32) -> Result<()> {
        let areas =
            ui::workspace_areas(self.screen, self.focus, &self.workspace);
        let focus = areas.get(self.focus).context("Focused pane is hidden")?;
        let Some(boundary) = areas.boundaries.iter().rev().find(|boundary| {
            boundary.first.intersection(focus) == focus
                || boundary.second.intersection(focus) == focus
        }) else {
            self.status =
                "Expand another pane or leave fullscreen to resize a split"
                    .into();
            return Ok(());
        };
        let signed = if boundary.first.intersection(focus) == focus {
            delta
        } else {
            delta.saturating_neg()
        };
        if let Some(tree) = &mut self.workspace.layout {
            tree.resize_path(&boundary.path, boundary.adjusted_percent(signed));
        }
        self.commit_panel_layout()
    }

    pub(super) fn focus_spatially(&mut self, edge: Dock) {
        self.prepare_panel_focus();
        if let Some(pane) = self.spatial_neighbor(edge) {
            self.focus = pane;
        }
    }

    fn spatial_neighbor(&self, edge: Dock) -> Option<Pane> {
        let areas =
            ui::workspace_areas(self.screen, self.focus, &self.workspace);
        if let Some(pane) = neighbor(&areas, self.focus, edge) {
            return Some(pane);
        }
        if Pane::ALL.into_iter().all(|pane| {
            !self.workspace.pane_visible(pane) || areas.get(pane).is_some()
        }) {
            return None;
        }
        // Resolve responsive suppression against the same topology at a size
        // that fits all four leaves, even at the minimum 15% split ratios.
        // Explicitly collapsed panes remain excluded by workspace_areas.
        let expanded = ui::workspace_areas(
            Rect::new(0, 0, 2048, 1024),
            self.focus,
            &self.workspace,
        );
        neighbor(&expanded, self.focus, edge)
            .filter(|pane| areas.get(*pane).is_none())
    }
}

fn neighbor(areas: &ui::Areas, focus: Pane, edge: Dock) -> Option<Pane> {
    let current = areas.get(focus)?;
    Pane::ALL
        .into_iter()
        .filter(|pane| *pane != focus)
        .filter_map(|pane| {
            let area = areas.get(pane)?;
            neighbor_distance(current, area, edge)
                .map(|distance| (pane, distance))
        })
        .min_by_key(|(_, distance)| *distance)
        .map(|(pane, _)| pane)
}

fn neighbor_distance(
    mut current: Rect,
    mut candidate: Rect,
    edge: Dock,
) -> Option<(bool, u16, u32)> {
    // Normalize vertical motion to the horizontal axis. A pane above/below
    // cannot become a left/right target merely because its center differs.
    if !edge.horizontal() {
        current =
            Rect::new(current.y, current.x, current.height, current.width);
        candidate = Rect::new(
            candidate.y,
            candidate.x,
            candidate.height,
            candidate.width,
        );
    }
    let gap = match edge {
        Dock::Left | Dock::Above => current.x.checked_sub(candidate.right())?,
        Dock::Right | Dock::Below => {
            candidate.x.checked_sub(current.right())?
        }
    };
    let aligned =
        current.y < candidate.bottom() && candidate.y < current.bottom();
    let current_center = u32::from(current.y) + u32::from(current.bottom());
    let candidate_center =
        u32::from(candidate.y) + u32::from(candidate.bottom());
    Some((!aligned, gap, current_center.abs_diff(candidate_center)))
}
