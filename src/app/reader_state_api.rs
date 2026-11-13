//! Application entry points delegate local invariants to the reader.
use super::{App, ReaderIdentity};
impl App {
    /// Current readable-text byte offset, independent of soft wrapping.
    pub fn cursor_offset(&self) -> usize {
        self.reader.cursor_offset()
    }

    pub(super) fn clamp_cursor(&mut self) {
        self.reader.clamp_cursor()
    }

    pub(super) fn place_cursor(&mut self, offset: usize) {
        self.reader.place_cursor(offset)
    }

    pub(super) fn ensure_cursor_visible(&mut self) {
        self.reader.ensure_cursor_visible()
    }

    pub(super) fn line_edge(&mut self, end: bool) {
        self.reader.line_edge(end)
    }

    /// Inclusive visual selection as a half-open, grapheme-safe byte range.
    pub fn selected_range(&self) -> Option<std::ops::Range<usize>> {
        self.reader.selected_range()
    }

    pub(crate) fn horizontal_for_row(&self, row: usize) -> usize {
        self.reader.horizontal_for_row(row)
    }

    pub(super) fn scroll_horizontal_row(&mut self, row: usize, right: bool) {
        self.reader.scroll_horizontal_row(row, right)
    }

    pub(super) fn reading_article(&self) -> Option<i64> {
        self.reader.reading_article()
    }

    /// Identity of the publisher source retained during asynchronous navigation.
    pub fn reader_identity(&self) -> Option<ReaderIdentity> {
        self.reader.reader_identity()
    }

    /// Whether syntax or layout preparation still has admitted work.
    pub fn reader_pending(&self) -> bool {
        self.reader.reader_pending()
    }

    pub(super) fn capture_reader(&self) -> super::reader_state::ReaderSnapshot {
        self.reader.capture_reader()
    }

    pub(super) fn restore_reader(
        &mut self,
        snapshot: super::reader_state::ReaderSnapshot,
    ) {
        let selected = self.current().map(|article| article.id);
        self.reader.restore_reader(snapshot, selected)
    }

    pub(super) fn clear_current(&mut self) {
        self.reader.clear_current(self.render_context())
    }

    pub(super) fn row_byte_at(&self, row: usize, goal: usize) -> usize {
        self.reader.row_byte_at(row, goal)
    }

    pub(super) fn clamp_scroll(&mut self) {
        self.reader.clamp_scroll()
    }

    pub(super) fn restore_anchor(&mut self, anchor: usize) {
        self.reader.restore_anchor(anchor)
    }

    pub(super) fn apply_reader_justification(&mut self) {
        self.reader.apply_justification(self.workspace.justify)
    }

    pub(super) fn select_first_annotation(&mut self) {
        self.reader.select_first_annotation()
    }

    /// Selected position in resolved annotation order, if any notes exist.
    pub fn annotation_selected(&self) -> Option<usize> {
        self.reader.annotation_selected()
    }

    pub(super) fn move_annotation(&mut self, forward: bool, distance: usize) {
        self.reader.move_annotation(forward, distance)
    }

    /// Annotation identities beginning on this wrapped display row.
    pub fn reader_note_markers(&self, row: usize) -> &[i64] {
        self.reader.reader_note_markers(row)
    }

    /// Stable hovered note identity and popup origin, without parsing or I/O.
    pub fn hovered_note(&self) -> Option<(i64, ratatui::layout::Position)> {
        self.reader.hovered_note()
    }

    pub(super) fn note_at_offset(&self, offset: usize) -> Option<i64> {
        self.reader.note_at_offset(offset)
    }

    pub(super) fn move_word(
        &mut self,
        motion: crate::reader::motion::WordMotion,
        wide: bool,
        count: usize,
    ) {
        self.reader.move_word(motion, wide, count);
    }

    pub(super) fn move_characters(&mut self, forward: bool, count: usize) {
        self.reader.move_characters(forward, count);
    }

    pub(super) fn goto_column(&mut self, column: usize) {
        self.reader.goto_column(column);
    }

    pub(super) fn control_destination(
        &mut self,
        code: crossterm::event::KeyCode,
        count: usize,
    ) {
        self.reader
            .control_destination(code, count, !self.reader_stale());
    }

    pub(super) fn first_nonblank(&mut self) {
        self.reader.first_nonblank();
    }

    pub(super) fn goto_reader_row(&mut self, row: usize) {
        self.reader.goto_reader_row(row, !self.reader_stale());
    }

    pub(super) fn viewport_row(
        &mut self,
        code: crossterm::event::KeyCode,
        count: usize,
    ) {
        self.reader.viewport_row(code, count, !self.reader_stale());
    }

    pub(super) fn vim_g_key(
        &mut self,
        code: crossterm::event::KeyCode,
        count: Option<usize>,
    ) {
        self.reader.vim_g_key(code, count, !self.reader_stale());
        if let Some(status) = self.reader.take_motion_status() {
            self.status = status;
        }
    }

    pub(super) fn vim_z_key(
        &mut self,
        code: crossterm::event::KeyCode,
        count: usize,
    ) {
        self.reader.vim_z_key(code, count);
        if let Some(status) = self.reader.take_motion_status() {
            self.status = status;
        }
    }

    pub(super) fn vim_object(
        &mut self,
        code: crossterm::event::KeyCode,
        around: bool,
    ) {
        self.reader.vim_object(code, around);
    }

    pub(super) fn find_character(
        &mut self,
        find: super::vim::Find,
        count: usize,
        repeat: bool,
    ) {
        self.reader.find_character(find, count, repeat);
        if let Some(status) = self.reader.take_motion_status() {
            self.status = status;
        }
    }

    pub(super) fn paragraph(&mut self, forward: bool, count: usize) {
        self.reader.paragraph(forward, count);
    }

    pub(super) fn percent_motion(&mut self, count: Option<usize>) {
        self.reader.percent_motion(count, !self.reader_stale());
    }

    pub(super) fn scroll_reader(&mut self, forward: bool, count: usize) {
        self.reader.scroll_reader(forward, count);
    }

    pub(super) fn move_reader(&mut self, forward: bool, amount: usize) {
        let motion = self.pending_formula_motion();
        self.reader.move_reader(forward, amount);
        self.retarget_formula_motion(motion);
    }

    pub(super) fn render_context(&self) -> super::reader_state::RenderContext {
        super::reader_state::RenderContext {
            annotated: self.view == super::View::Annotated,
            immediate: self.immediate_layout,
            justify: self.workspace.justify,
            options: self.formula_options(),
        }
    }

    pub(super) fn wrap_reader(&mut self) {
        self.reader.wrap_reader(self.render_context());
    }

    /// Reflow only when the reader geometry changes, retaining source anchors.
    pub fn resize_reader(&mut self, width: usize, height: usize) {
        self.reader.resize_reader(
            width,
            height,
            self.render_context(),
            !self.navigation_loading && !self.projecting_navigation,
        );
    }
}
