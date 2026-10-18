//! Elapsed-time pane translation shared by held-neighbor and release motion.

use std::time::Instant;

use ratatui::layout::Rect;

use super::{FRAME_INTERVAL, SETTLE_DURATION, eased_coordinate};

pub(in crate::app) struct Slide {
    from: Rect,
    pub(in crate::app) to: Rect,
    pub(in crate::app) current: Rect,
    started: Instant,
    last_frame: Instant,
}

impl Slide {
    pub(in crate::app) fn new(area: Rect) -> Self {
        let now = Instant::now();
        Self {
            from: area,
            to: area,
            current: area,
            started: now,
            last_frame: now,
        }
    }

    pub(in crate::app) fn active(&self) -> bool {
        self.current != self.to
    }

    pub(in crate::app) fn retarget(&mut self, to: Rect, now: Instant) {
        if self.to == to {
            return;
        }
        self.from = self.current;
        self.to = to;
        self.started = now;
        self.last_frame = now;
    }

    pub(in crate::app) fn tick(&mut self, now: Instant) -> bool {
        if !self.active()
            || now.saturating_duration_since(self.last_frame) < FRAME_INTERVAL
        {
            return false;
        }
        self.last_frame = now;
        let elapsed = now.saturating_duration_since(self.started);
        if elapsed >= SETTLE_DURATION {
            self.current = self.to;
            return true;
        }
        self.current.x = eased_coordinate(self.from.x, self.to.x, elapsed);
        self.current.y = eased_coordinate(self.from.y, self.to.y, elapsed);
        true
    }
}
