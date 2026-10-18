//! Frozen neighboring panes make room without changing durable workspace state.

use std::time::Instant;

use ratatui::{buffer::Buffer, layout::Rect};

use crate::{model::Pane, ui::Areas};

use super::{FloatingPane, Slide, snapshot};

// All pane pictures together remain bounded even in unusually large terminals.
const MAX_WORKSPACE_CELLS: usize = 40_000;

pub(in crate::app) struct Neighbors {
    pub(in crate::app) region: Rect,
    pub(in crate::app) gap: Rect,
    slots: Vec<(Pane, Rect)>,
    pictures: Vec<Neighbor>,
}

struct Neighbor {
    pane: Pane,
    cells: Buffer,
    slide: Slide,
}

impl Neighbors {
    pub(in crate::app) fn capture(
        frame: &Buffer,
        slots: &[(Pane, Rect)],
        held: Pane,
    ) -> Option<Self> {
        let region = slots.iter().map(|(_, area)| *area).reduce(Rect::union)?;
        if usize::from(region.width) * usize::from(region.height)
            > MAX_WORKSPACE_CELLS
        {
            return None;
        }
        let mut pictures = Vec::new();
        for (pane, area) in slots.iter().filter(|(pane, _)| *pane != held) {
            pictures.push(Neighbor {
                pane: *pane,
                cells: snapshot(frame, *area)?,
                slide: Slide::new(*area),
            });
        }
        Some(Self {
            region,
            gap: slots.iter().find(|(pane, _)| *pane == held)?.1,
            slots: slots.to_vec(),
            pictures,
        })
    }

    pub(in crate::app) fn preview(&mut self, order: [Pane; 4], now: Instant) {
        let mut column = self.region.x;
        for pane in order {
            let Some((_, home)) = self.slots.iter().find(|(id, _)| *id == pane)
            else {
                continue;
            };
            let area = Rect::new(column, home.y, home.width, home.height);
            column += home.width;
            match self.pictures.iter_mut().find(|item| item.pane == pane) {
                Some(item) => item.slide.retarget(area, now),
                None => self.gap = area,
            }
        }
    }

    pub(in crate::app) fn settle(
        mut self,
        areas: &Areas,
        now: Instant,
    ) -> Option<Self> {
        // Reflow or responsive visibility changes need real content rendering.
        if self.slots.iter().any(|(pane, home)| {
            areas
                .get(*pane)
                .is_none_or(|area| area.as_size() != home.as_size())
        }) {
            return None;
        }
        for item in &mut self.pictures {
            if let Some(area) = areas.get(item.pane) {
                item.slide.retarget(area, now);
            }
        }
        Some(self)
    }

    pub(in crate::app) fn pictures(
        &self,
    ) -> impl Iterator<Item = FloatingPane<'_>> {
        self.pictures.iter().map(|item| FloatingPane {
            pane: item.pane,
            slot: item.cells.area,
            area: item.slide.current,
            cells: &item.cells,
        })
    }

    pub(in crate::app) fn active(&self) -> bool {
        self.pictures.iter().any(|item| item.slide.active())
    }

    pub(in crate::app) fn tick(&mut self, now: Instant) -> bool {
        self.pictures
            .iter_mut()
            .fold(false, |changed, item| item.slide.tick(now) | changed)
    }
}
