use super::ReaderState;

impl ReaderState {
    pub(in crate::app) fn scroll_reader(
        &mut self,
        forward: bool,
        count: usize,
    ) {
        self.viewport.pending_anchor = None;
        self.viewport.first = if forward {
            self.viewport.first.saturating_add(count)
        } else {
            self.viewport.first.saturating_sub(count)
        };
        self.viewport.first = self.viewport.first.min(
            self.projection
                .lines
                .len()
                .saturating_sub(self.viewport.height),
        );
        self.viewport.row = self.viewport.row.clamp(
            self.viewport.first,
            (self.viewport.first + self.viewport.height)
                .min(self.projection.lines.len())
                .saturating_sub(1)
                .max(self.viewport.first),
        );
        self.clamp_cursor();
    }
}
