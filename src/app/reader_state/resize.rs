//! Geometry transitions retain source anchors while layout prepares.
use super::ReaderState;
impl ReaderState {
    pub(in crate::app) fn resize_reader(
        &mut self,
        width: usize,
        height: usize,
        context: super::RenderContext,
        can_project: bool,
    ) {
        let width = width.max(1);
        if self.viewport.width == width
            && self.viewport.height == height.max(1)
            && self.viewport.pending_anchor.is_none()
        {
            return;
        }
        let cursor = self.cursor_offset();
        let pending = self.viewport.pending_anchor.take();
        let anchor = pending.unwrap_or_else(|| {
            self.projection
                .source_ranges
                .get(self.viewport.first)
                .map_or(0, |range| range.start)
        });
        self.viewport.height = height.max(1);
        let reflow = self.viewport.width != width;
        if reflow {
            self.viewport.width = width;
            if can_project {
                self.wrap_reader(context);
            }
        }
        if self.source_preparing() {
            self.viewport.pending_reflow |= pending.is_none() && reflow;
            self.viewport.pending_anchor = Some(anchor);
            self.clamp_scroll();
            return;
        }
        let preserve_cursor =
            self.viewport.pending_reflow || reflow && pending.is_none();
        self.viewport.pending_reflow = false;
        if reflow || pending.is_some() {
            self.restore_anchor(anchor);
        }
        if preserve_cursor {
            self.place_cursor(cursor);
            self.ensure_cursor_visible();
        }
        self.clamp_scroll();
    }
}
