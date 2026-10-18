//! Spatial keyboard operations delegate to the same docking contract as mouse.

use anyhow::{Context, Result};

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
            -delta
        };
        if let Some(tree) = &mut self.workspace.layout {
            tree.adjust_split(&boundary.path, signed);
        }
        self.commit_panel_layout()
    }

    pub(super) fn focus_spatially(&mut self, edge: Dock) {
        if let Some(pane) = self.spatial_neighbor(edge) {
            self.focus = pane;
            self.selection = None;
        }
    }

    fn spatial_neighbor(&self, edge: Dock) -> Option<Pane> {
        let areas =
            ui::workspace_areas(self.screen, self.focus, &self.workspace);
        let current = areas.get(self.focus)?;
        let center = (
            i32::from(current.x) * 2 + i32::from(current.width),
            i32::from(current.y) * 2 + i32::from(current.height),
        );
        areas
            .ordered()
            .into_iter()
            .filter(|(pane, _)| *pane != self.focus)
            .filter_map(|(pane, area)| {
                let x =
                    i32::from(area.x) * 2 + i32::from(area.width) - center.0;
                let y =
                    i32::from(area.y) * 2 + i32::from(area.height) - center.1;
                let eligible = match edge {
                    Dock::Left => x < 0,
                    Dock::Right => x > 0,
                    Dock::Above => y < 0,
                    Dock::Below => y > 0,
                };
                eligible.then_some((pane, x.abs() + y.abs() * 2))
            })
            .min_by_key(|(_, distance)| *distance)
            .map(|(pane, _)| pane)
    }
}
