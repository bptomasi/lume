//! A move captures stable hit regions; background article reloads cannot abort it.

mod collision;

use std::time::Instant;

use crossterm::event::MouseEvent;
use ratatui::buffer::Buffer;

use crate::app::{FloatingPane, pane_motion};
use ratatui::layout::{Position, Rect};

use crate::{model::Pane, presentation::geometry};

use super::{App, Gesture, PanelDrag};

pub(super) struct MoveCapture {
    pub(super) preview: PanelDrag,
    tabs: Vec<(Pane, Rect)>,
    origin: Rect,
    viewport: Rect,
    grab: Position,
    pointer: Position,
    dragged: bool,
    custom: bool,
    compact: bool,
    picture: Option<Buffer>,
    captured: bool,
    slots: Vec<(Pane, Rect)>,
    order: [Pane; 4],
    pub(super) neighbors: Option<pane_motion::Neighbors>,
}

impl MoveCapture {
    pub(super) fn update(&mut self, event: MouseEvent) {
        let position = Position::new(event.column, event.row);
        self.dragged |= position.x.abs_diff(self.grab.x) >= 2
            || position.y.abs_diff(self.grab.y) >= 1;
        self.pointer = position;
        if !self.dragged {
            return;
        }
        let previous = self.preview;
        self.preview.target = self.target(position);
        self.preview.edge = self.dock_edge(position, self.preview.target);
        if self.preview != previous {
            self.preview_neighbors();
        }
    }

    fn target(&self, pointer: Position) -> Option<Pane> {
        if !self.viewport.contains(pointer) {
            return None;
        }
        if let Some((pane, _)) =
            self.tabs.iter().find(|(_, area)| area.contains(pointer))
        {
            return (*pane != self.preview.pane).then_some(*pane);
        }
        if let Some((pane, _)) = self.slots.iter().find(|(pane, area)| {
            *pane != self.preview.pane
                && area.contains(pointer)
                && (pointer.y > area.y && pointer.y < area.y + 3
                    || pointer.y >= area.bottom().saturating_sub(3))
        }) {
            return Some(*pane);
        }
        if self.custom {
            return self
                .slots
                .iter()
                .find(|(pane, area)| {
                    *pane != self.preview.pane && area.contains(pointer)
                })
                .map(|(pane, _)| *pane);
        }
        collision::target(
            &self.slots,
            self.origin,
            self.drag_area(),
            self.preview.target,
        )
    }

    fn dock_edge(
        &self,
        pointer: Position,
        target: Option<Pane>,
    ) -> Option<crate::model::Dock> {
        let (_, area) = self.slots.iter().find(|(pane, area)| {
            Some(*pane) == target && area.contains(pointer)
        })?;
        if pointer.y > area.y && pointer.y < area.y + 3 {
            return Some(crate::model::Dock::Above);
        }
        if pointer.y >= area.bottom().saturating_sub(3) {
            return Some(crate::model::Dock::Below);
        }
        self.custom
            .then_some(if pointer.x < area.x + area.width / 2 {
                crate::model::Dock::Left
            } else {
                crate::model::Dock::Right
            })
    }

    pub(super) fn capture(&mut self, frame: &Buffer) {
        if self.captured {
            return;
        }
        self.captured = true;
        self.compact = u32::from(self.origin.width) * 4
            > u32::from(self.viewport.width) * 3
            || u32::from(self.origin.width) * u32::from(self.origin.height)
                > 20_000;
        self.picture = if self.compact {
            Some(crate::presentation::pane_picture::compact(
                frame,
                self.origin,
                self.preview.pane,
            ))
        } else {
            pane_motion::snapshot(frame, self.origin)
        };
        if self.picture.is_some() && !self.custom {
            self.neighbors = pane_motion::Neighbors::capture(
                frame,
                &self.slots,
                self.preview.pane,
            );
            self.preview_neighbors();
        }
    }

    fn preview_neighbors(&mut self) {
        let target =
            self.preview.target.filter(|_| self.preview.edge.is_none());
        let order = target.map_or(self.order, |target| {
            crate::app::panels::reordered(self.order, self.preview.pane, target)
        });
        if let Some(neighbors) = &mut self.neighbors {
            neighbors.preview(order, Instant::now());
        }
    }

    fn settle(self, app: &mut App) {
        let areas = geometry::workspace_areas(
            app.layout.screen(),
            app.layout.focus(),
            app.layout.workspace(),
        );
        let from = self
            .floating()
            .map_or_else(|| self.drag_area(), |picture| picture.area);
        let flight = if let (Some(picture), Some(to)) =
            (self.picture, areas.get(self.preview.pane))
        {
            pane_motion::PaneFlight::start(self.preview.pane, picture, from, to)
        } else {
            None
        };
        let neighbors = self
            .neighbors
            .and_then(|neighbors| neighbors.settle(&areas, Instant::now()));
        app.pane_motion.settle(flight, neighbors);
    }

    pub(super) fn floating(&self) -> Option<FloatingPane<'_>> {
        if !self.dragged {
            return None;
        }
        let picture = self.picture.as_ref()?;
        let mut area = self.drag_area();
        if self.compact {
            area.width = picture.area.width;
            area.height = picture.area.height;
            area.x = self
                .pointer
                .x
                .saturating_sub(area.width.saturating_sub(6))
                .clamp(
                    self.viewport.x,
                    self.viewport.right().saturating_sub(area.width),
                );
        }
        Some(FloatingPane {
            pane: self.preview.pane,
            slot: self.origin,
            area,
            cells: picture,
        })
    }

    fn drag_area(&self) -> Rect {
        // Keep the grabbed title reachable; the picture may extend beyond
        // the viewport and is clipped by the renderer, never squeezed to fit.
        let x = i32::from(self.origin.x) + i32::from(self.pointer.x)
            - i32::from(self.grab.x);
        let y = i32::from(self.origin.y) + i32::from(self.pointer.y)
            - i32::from(self.grab.y);
        Rect::new(
            x.clamp(
                i32::from(self.viewport.x),
                i32::from(self.viewport.right().saturating_sub(1)),
            ) as u16,
            y.clamp(
                i32::from(self.viewport.y),
                i32::from(self.viewport.bottom().saturating_sub(1)),
            ) as u16,
            self.origin.width,
            self.origin.height,
        )
    }

    pub(super) fn hint(&self) -> String {
        let name = self.preview.pane.title();
        match self.preview.target {
            Some(target) => format!(
                "Move {name} to {} · release saves · Esc cancels",
                target.title()
            ),
            None => {
                format!("Move {name} · drag to a pane or top tab · Esc cancels")
            }
        }
    }

    pub(super) fn completed_hint(&self) -> String {
        format!("{} moved · layout saved", self.preview.pane.title())
    }
}

impl App {
    pub(super) fn complete_reorder(
        &mut self,
        capture: MoveCapture,
    ) -> anyhow::Result<()> {
        let pane = capture.preview.pane;
        if !capture.dragged {
            self.layout.focus_pane(pane);
            self.resize_workspace(self.layout.screen());
            self.interaction.report(format!("{} focused", pane.title()));
            return Ok(());
        }
        let Some(target) = capture.preview.target else {
            self.interaction
                .report("Panel move canceled · no destination".into());
            capture.settle(self);
            return Ok(());
        };
        if capture.custom || capture.preview.edge.is_some() {
            let expanded = self.layout.workspace().pane_visible(target);
            self.dock_pane(
                pane,
                target,
                capture.preview.edge.unwrap_or(crate::model::Dock::Right),
            )?;
            self.layout.show_pane(target, expanded);
        } else {
            self.move_pane_to(pane, target);
        }
        self.commit_panel_layout()?;
        self.interaction.report(capture.completed_hint());
        capture.settle(self);
        Ok(())
    }

    pub(super) fn begin_reorder(
        &mut self,
        pane: Pane,
        areas: &geometry::Areas,
        event: MouseEvent,
    ) {
        let Some(origin) = areas.get(pane) else {
            return;
        };
        let tabs = if self.layout.workspace().fullscreen {
            Vec::new()
        } else {
            geometry::pane_buttons(
                self.layout.screen(),
                self.layout.workspace(),
            )
        };
        let capture = MoveCapture {
            preview: PanelDrag {
                pane,
                target: None,
                edge: None,
            },
            tabs,
            origin,
            viewport: self.layout.screen(),
            grab: Position::new(event.column, event.row),
            pointer: Position::new(event.column, event.row),
            dragged: false,
            custom: self.layout.workspace().layout.is_some(),
            compact: false,
            picture: None,
            captured: false,
            slots: areas.ordered(),
            order: self.layout.workspace().panel_order,
            neighbors: None,
        };
        // Preserve focus so narrow layouts do not replace a visible target
        // when a header is pressed. Geometry remains fixed until release.
        self.reader.cancel_selection();
        self.interaction.vim_input().clear_input();
        self.interaction.set_panel_prefix(false);
        self.interaction.report(capture.hint());
        self.pointer.gesture = Some(Gesture::Reorder(capture));
    }
}
