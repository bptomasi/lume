use super::ReaderState;

impl ReaderState {
    pub(in crate::app) fn move_reader(&mut self, forward: bool, amount: usize) {
        self.viewport.pending_anchor = None;
        let goal = self.viewport.goal.unwrap_or_else(|| {
            self.row_column(self.viewport.row, self.viewport.byte)
        });
        self.viewport.goal = Some(goal);
        self.viewport.row = if forward {
            self.viewport
                .row
                .saturating_add(amount)
                .min(self.projection.lines.len().saturating_sub(1))
        } else {
            self.viewport.row.saturating_sub(amount)
        };
        self.viewport.byte = self.row_byte_at(self.viewport.row, goal);
        self.ensure_cursor_visible();
    }
}
