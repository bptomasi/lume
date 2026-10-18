//! Bounded presentation snapshots follow the pointer without reflow or SQL.
//!
//! Ratatui owns cell storage and rendering. Neighbors open an insertion gap,
//! then settle with the held pane after release. Elapsed-time transitions skip
//! late frames. New input or geometry cancels motion; commits never wait.

mod neighbors;
mod slide;

use std::time::{Duration, Instant};

pub(super) use neighbors::Neighbors;
use slide::Slide;

use ratatui::{buffer::Buffer, layout::Rect};

use crate::model::Pane;

use super::App;

// Bound capture/copy work; larger panes retain the existing target preview.
const MAX_SNAPSHOT_CELLS: usize = 20_000;
const SETTLE_DURATION: Duration = Duration::from_millis(160);
const FRAME_INTERVAL: Duration = Duration::from_millis(16);

/// Borrowed picture of a held or settling pane, already using its theme colors.
pub struct FloatingPane<'a> {
    /// Pane represented by this picture.
    pub pane: Pane,
    /// Layout slot masked while its picture floats over neighboring panes.
    pub slot: Rect,
    /// Current position; the picture is clipped at the terminal edges.
    pub area: Rect,
    /// Frozen original cells; article layout is not recalculated per motion.
    pub cells: &'a Buffer,
}

pub(super) struct PaneFlight {
    pane: Pane,
    picture: Buffer,
    slide: Slide,
}

impl PaneFlight {
    pub(super) fn start(
        pane: Pane,
        picture: Buffer,
        from: Rect,
        to: Rect,
    ) -> Option<Self> {
        // A dimension change needs actual reader reflow, not stretched cells.
        if from.as_size() != to.as_size() {
            return None;
        }
        let mut slide = Slide::new(from);
        slide.retarget(to, Instant::now());
        Some(Self {
            pane,
            picture,
            slide,
        })
    }

    fn floating(&self) -> FloatingPane<'_> {
        FloatingPane {
            pane: self.pane,
            slot: self.slide.to,
            area: self.slide.current,
            cells: &self.picture,
        }
    }
}

impl App {
    /// Capture the held pane and bounded neighbors once per gesture.
    pub fn capture_pane_frame(&mut self, frame: &Buffer) {
        self.capture_drag_frame(frame);
    }

    /// Read pointer-following or settling presentation, without changing layout.
    pub fn floating_pane(&self) -> Option<FloatingPane<'_>> {
        self.pane_flight
            .as_ref()
            .map(PaneFlight::floating)
            .or_else(|| self.floating_drag())
    }

    /// Neighboring pane pictures, below the held pane, in bounded storage.
    pub fn neighboring_panes(&self) -> impl Iterator<Item = FloatingPane<'_>> {
        self.neighbors().into_iter().flat_map(Neighbors::pictures)
    }

    /// Region replaced by captured neighbors while their positions are animated.
    pub fn pane_motion_region(&self) -> Option<Rect> {
        self.neighbors().map(|motion| motion.region)
    }

    /// Candidate empty slot for the held pane, without a durable layout edit.
    pub fn pane_insertion_slot(&self) -> Option<Rect> {
        self.drag_neighbors().map(|motion| motion.gap)
    }

    fn neighbors(&self) -> Option<&Neighbors> {
        self.pane_neighbors
            .as_ref()
            .or_else(|| self.drag_neighbors())
    }

    /// Whether pending pane translations need another animation frame.
    pub fn pane_motion_active(&self) -> bool {
        self.pane_flight.is_some()
            || self.neighbors().is_some_and(Neighbors::active)
    }

    /// Advance elapsed-time translations; delayed frames never build a queue.
    pub fn tick_pane_motion(&mut self, now: Instant) -> bool {
        let mut changed = self.tick_drag_neighbors(now);
        if let Some(neighbors) = &mut self.pane_neighbors {
            changed |= neighbors.tick(now);
        }
        if let Some(flight) = &mut self.pane_flight {
            changed |= flight.slide.tick(now);
            // A stationary held picture still covers its gap while the
            // remaining neighbors finish returning or making room.
            if !flight.slide.active()
                && !self.pane_neighbors.as_ref().is_some_and(Neighbors::active)
            {
                self.pane_flight = None;
                changed = true;
            }
        }
        if self.pane_flight.is_none() && self.pane_neighbors.take().is_some() {
            changed = true;
        }
        changed
    }
}

pub(super) fn snapshot(frame: &Buffer, area: Rect) -> Option<Buffer> {
    let cells = usize::from(area.width) * usize::from(area.height);
    if area.is_empty()
        || cells > MAX_SNAPSHOT_CELLS
        || frame.area.intersection(area) != area
    {
        return None;
    }
    let mut picture = Buffer::empty(area);
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            picture[(x, y)] = frame[(x, y)].clone();
        }
    }
    Some(picture)
}

fn eased_coordinate(from: u16, to: u16, elapsed: Duration) -> u16 {
    let total = SETTLE_DURATION.as_millis() as i64;
    let remaining = total - elapsed.as_millis().min(total as u128) as i64;
    let progress = total.pow(3) - remaining.pow(3);
    let distance = i64::from(to) - i64::from(from);
    (i64::from(from) + distance * progress / total.pow(3)) as u16
}

#[cfg(test)]
mod tests;
