//! Inputs admitted by application coordinators, with reader invariants intact.

use crate::{
    model::{Article, Comment, ContentSource, FetchPolicy},
    reader::cursor,
};

use super::{ReaderState, RenderContext};

/// Remembered anchors already qualified against the incoming cache revision.
#[derive(Default)]
pub(in crate::app) struct RememberedProgress {
    pub(in crate::app) feed: Option<usize>,
    pub(in crate::app) full: Option<usize>,
}

impl ReaderState {
    pub(in crate::app) fn open_document(
        &mut self,
        article: Article,
        data: &mut crate::app::reading_data::ReadingData,
        preferred: ContentSource,
        context: RenderContext,
        progress: RememberedProgress,
    ) -> anyhow::Result<()> {
        anyhow::ensure!(
            article.id == data.article,
            "Reader payload does not match its publisher metadata",
        );
        self.context = context;
        self.viewport.pending_reflow = false;
        self.publisher.article = Some(article);
        self.apply_article_content(data, preferred);
        self.replace_comments(std::mem::take(&mut data.comments));
        self.prepare_reader_metadata();
        let anchor = if self.publisher.source == ContentSource::Full {
            progress.full.unwrap_or_else(|| {
                self.publisher
                    .cache
                    .as_ref()
                    .map_or(0, |document| document.progress)
            })
        } else {
            progress.feed.unwrap_or(data.progress)
        };
        self.begin_document(anchor, context);
        Ok(())
    }

    fn begin_document(&mut self, anchor: usize, context: RenderContext) {
        self.viewport.pending_anchor = Some(anchor);
        self.viewport.first = 0;
        self.viewport.row = 0;
        self.viewport.byte = 0;
        self.viewport.goal = None;
        self.viewport.selection = None;
        self.viewport.horizontal = 0;
        self.viewport.horizontal_state = super::Horizontal::default();
        self.wrap_reader(context);
        self.restore_anchor(anchor);
        self.clamp_scroll();
    }

    pub(in crate::app) fn replace_comments(&mut self, comments: Vec<Comment>) {
        self.notes.comments = comments;
        self.notes.selected = self
            .notes
            .selected
            .min(self.notes.comments.len().saturating_sub(1));
        self.rebuild_annotations();
    }

    pub(in crate::app) fn select_comment(&mut self, index: usize) {
        self.notes.selected =
            index.min(self.notes.comments.len().saturating_sub(1));
    }

    pub(in crate::app) fn select_comment_id(&mut self, id: i64) {
        if let Some(index) =
            self.notes.comments.iter().position(|note| note.id == id)
        {
            self.select_comment(index);
        }
    }

    pub(in crate::app) fn point_cursor(&mut self, row: usize, byte: usize) {
        self.viewport.row = row;
        self.viewport.byte = byte;
        self.viewport.goal = None;
        self.clamp_cursor();
    }

    pub(in crate::app) fn reveal_row(&mut self, row: usize) {
        self.point_cursor(row, 0);
        self.viewport.first = self.viewport.row;
        self.clamp_scroll();
    }

    pub(in crate::app) fn cancel_restore(&mut self) {
        self.viewport.pending_anchor = None;
    }

    pub(in crate::app) fn cancel_selection(&mut self) -> bool {
        self.viewport.selection.take().is_some()
    }

    pub(in crate::app) fn select_from(
        &mut self,
        anchor: usize,
        linewise: bool,
    ) {
        self.viewport.selection = Some(cursor::boundary(
            &self.projection.text,
            anchor.min(self.projection.text.len()),
        ));
        self.viewport.linewise = linewise;
    }

    pub(in crate::app) fn toggle_selection(&mut self, linewise: bool) {
        if self.viewport.selection.is_some()
            && self.viewport.linewise == linewise
        {
            self.cancel_selection();
        } else {
            self.select_from(self.cursor_offset(), linewise);
        }
        self.viewport.linewise = linewise;
    }

    pub(in crate::app) fn set_fetch_policy(&mut self, policy: FetchPolicy) {
        self.publisher.fetch_policy = policy;
    }

    pub(in crate::app) fn take_motion_status(&mut self) -> Option<String> {
        self.motion_status.take()
    }

    pub(super) fn source_preparing(&self) -> bool {
        self.renderer
            .preparing(&self.publisher.body, self.viewport.width)
    }

    pub(in crate::app) fn source_ready(&self) -> bool {
        self.renderer.source_ready(&self.publisher.body)
    }

    pub(in crate::app) fn poll_render_worker(&mut self) -> bool {
        self.renderer.poll()
    }

    pub(in crate::app) fn render_worker_stopped(&self) -> bool {
        self.renderer.is_stopped()
    }

    pub(in crate::app) fn enable_background_layout(
        &mut self,
    ) -> anyhow::Result<()> {
        self.renderer.enable_background_layout()
    }

    pub(in crate::app) fn prepare_source(&mut self, body: &str) -> bool {
        self.renderer.prepare_source(body, self.viewport.width)
    }
}

impl ReaderState {
    pub(in crate::app) fn feed_has_text(&self) -> bool {
        self.publisher.feed_kind == crate::content::BodyKind::Text
    }

    pub(in crate::app) fn reading_position(
        &self,
    ) -> Option<(crate::app::ReaderIdentity, usize)> {
        if self.context.annotated || self.projection.text.is_empty() {
            return None;
        }
        let identity = self.reader_identity()?;
        let anchor = self.viewport.pending_anchor.unwrap_or_else(|| {
            self.projection
                .source_ranges
                .get(self.viewport.first)
                .map_or(0, |range| range.start)
        });
        Some((identity, anchor))
    }
}

impl ReaderState {
    pub(in crate::app) fn selecting(&self) -> bool {
        self.viewport.selection.is_some()
    }

    pub(in crate::app) fn source(&self) -> ContentSource {
        self.publisher.source
    }

    pub(in crate::app) fn revision(&self) -> i64 {
        self.publisher.revision
    }

    pub(in crate::app) fn height(&self) -> usize {
        self.viewport.height
    }

    pub(in crate::app) fn cursor_row(&self) -> usize {
        self.viewport.row
    }

    pub(in crate::app) fn cursor_byte(&self) -> usize {
        self.viewport.byte
    }

    pub(in crate::app) fn first_row(&self) -> usize {
        self.viewport.first
    }

    pub(in crate::app) fn publisher_end(&self) -> usize {
        self.projection.article_end
    }
}
