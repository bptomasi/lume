//! Runtime network permission and the feed worker's single admission slot.

pub(super) struct NetworkState {
    demo: bool,
    enabled: bool,
    collecting: bool,
}

impl NetworkState {
    pub(super) fn new(demo: bool) -> Self {
        Self {
            demo,
            enabled: false,
            collecting: false,
        }
    }

    pub(super) fn demo(&self) -> bool {
        self.demo
    }

    pub(super) fn enable(&mut self, enabled: bool) {
        self.enabled = enabled && !self.demo;
    }

    pub(super) fn enabled(&self) -> bool {
        self.enabled
    }

    pub(super) fn collecting(&self) -> bool {
        self.collecting
    }

    pub(super) fn collection_started(&mut self) {
        self.collecting = true;
    }

    pub(super) fn collection_finished(&mut self) {
        self.collecting = false;
    }
}
