//! Reader cache, quote capture and durable reading position.

use anyhow::Result;
use ratatui::{
    style::{Color, Style},
    text::Line,
};

use super::{App, View};
use crate::content::{self, BodyKind};

/// In-memory viewport, reusable only while the displayed source stays intact.
pub(super) struct ReaderSnapshot {
    article: Option<i64>,
    comment: Option<i64>,
    source: crate::model::ContentSource,
    revision: i64,
    body: String,
    notes: Option<String>,
    cursor: usize,
    selection: Option<usize>,
    linewise: bool,
    scroll: usize,
    horizontal: usize,
    pending_anchor: Option<usize>,
}

impl App {
    pub(super) fn guard_stale_command(&self, verb: &str) -> Result<()> {
        if self.notebook.is_some() || !self.reader_stale() {
            return Ok(());
        }
        anyhow::ensure!(
            verb.starts_with("panel-")
                || matches!(
                    verb,
                    "help"
                        | "workspace"
                        | "fullscreen"
                        | "zen"
                        | "sidebar"
                        | "headlines"
                        | "reader"
                        | "annotations"
                        | "layout-undo"
                        | "layout-redo"
                        | "reading-width"
                        | "fullscreen-width"
                        | "quit"
                        | "q"
                        | "refresh"
                        | "r"
                        | "read"
                        | "favorites"
                        | "inbox"
                        | "all"
                        | "continue"
                        | "since-last-visit"
                        | "undo"
                ),
            "Wait for the selected article; the previous source is still displayed"
        );
        Ok(())
    }

    /// Apply ready syntax styles without changing text, cursor or selection.
    pub fn poll_reader(&mut self) -> bool {
        if !self.renderer.poll() {
            return false;
        }
        if self.navigation_loading || self.projecting_navigation {
            return false;
        }
        if self.renderer.is_stopped() {
            self.status =
                "Syntax worker stopped · plain code remains readable".into();
        }
        let cursor = self.cursor_offset();
        let anchor = self.pending_anchor.unwrap_or_else(|| {
            self.line_ranges
                .get(self.scroll)
                .map_or(0, |range| range.start)
        });
        let horizontal = self.horizontal;
        self.wrap_reader();
        self.horizontal = horizontal;
        if !self.renderer.preparing(&self.body, self.reader_width) {
            self.restore_ready_viewport(anchor, cursor);
        }
        self.clamp_scroll();
        true
    }

    fn restore_ready_viewport(&mut self, anchor: usize, cursor: usize) {
        let reflow = std::mem::take(&mut self.pending_reflow);
        if self.pending_anchor.take().is_some() && !reflow {
            self.restore_anchor(anchor);
            return;
        }
        self.scroll = self
            .line_ranges
            .iter()
            .position(|range| range.end > anchor)
            .unwrap_or(0);
        self.place_cursor(cursor);
        self.ensure_cursor_visible();
    }

    /// Whether the selected publisher source has not produced readable rows yet.
    pub fn article_preparing(&self) -> bool {
        self.navigation_loading
            || (self.view != View::Annotated
                && self.body_kind == BodyKind::Text
                && !self.renderer.source_ready(&self.body))
    }

    /// The previous article stays visible while a new selection is loading.
    /// Source-bound actions must wait for the matching content identity.
    pub fn reader_stale(&self) -> bool {
        self.reading_article.is_some()
            && (self.navigation_loading
                || self.projecting_navigation
                || self.current().map(|article| article.id)
                    != self.reading_article)
    }

    /// Metadata for the source actually displayed, including a retained preview.
    pub fn displayed_article(&self) -> Option<&crate::model::Article> {
        if self.reader_stale() {
            self.displayed_article.as_ref()
        } else {
            self.current()
        }
    }

    /// Whether the independent syntax worker still has admitted work.
    pub fn reader_pending(&self) -> bool {
        self.renderer.is_pending()
    }

    pub(super) fn capture_reader(&self) -> ReaderSnapshot {
        let cursor = self.cursor_offset();
        ReaderSnapshot {
            article: self.reading_article,
            comment: self
                .comments
                .get(self.comment_selected)
                .map(|note| note.id),
            source: self.content_source,
            revision: self.content_revision,
            body: self.body.clone(),
            notes: (cursor >= self.article_text_end)
                .then(|| self.reader_text.clone()),
            cursor,
            selection: self.selection,
            linewise: self.linewise,
            scroll: self.scroll,
            horizontal: self.horizontal,
            pending_anchor: self.pending_anchor,
        }
    }

    pub(super) fn restore_reader(&mut self, snapshot: ReaderSnapshot) {
        if snapshot.article.is_none()
            || self.current().map(|article| article.id) != snapshot.article
        {
            return;
        }
        if let Some(position) = self
            .comments
            .iter()
            .position(|note| Some(note.id) == snapshot.comment)
        {
            self.comment_selected = position;
        }
        if self.content_source != snapshot.source
            || self.content_revision != snapshot.revision
            || self.body != snapshot.body
            || snapshot
                .notes
                .is_some_and(|notes| notes != self.reader_text)
        {
            return;
        }
        self.selection = snapshot.selection;
        self.linewise = snapshot.linewise;
        self.pending_anchor = snapshot.pending_anchor;
        self.scroll = snapshot.scroll;
        self.horizontal = snapshot.horizontal;
        self.place_cursor(snapshot.cursor);
        self.ensure_cursor_visible();
    }

    pub(super) fn load_current_deferred(&mut self) -> Result<()> {
        self.cancel_reader_pointer();
        self.vim.clear_input();
        let Some(id) = self.current().map(|article| article.id) else {
            self.clear_current();
            return Ok(());
        };
        let feed = self.current().map_or(0, |article| article.feed_id);
        if self.navigation_worker.is_some() {
            self.defer_navigation(super::navigation_worker::Request::Article {
                id,
                feed,
            });
            return Ok(());
        }
        self.load_current()
    }

    pub(super) fn load_current(&mut self) -> Result<()> {
        if let Some(worker) = &mut self.navigation_worker {
            worker.cancel();
        }
        self.navigation_loading = false;
        self.projecting_navigation = false;
        self.cancel_reader_pointer();
        let Some(article) = self.current() else {
            self.clear_current();
            return Ok(());
        };
        let data = super::reading_data::ReadingData::load(
            &self.store,
            article.id,
            article.feed_id,
        )?;
        self.immediate_layout = true;
        let result = self.install_reading_data(data);
        self.immediate_layout = false;
        result
    }

    pub(super) fn install_reading_data(
        &mut self,
        mut data: super::reading_data::ReadingData,
    ) -> Result<()> {
        self.pending_reflow = false;
        self.displayed_article = self.current().cloned();
        self.reading_article = self.current().map(|article| article.id);
        self.apply_article_content(&mut data);
        self.comments = data.comments;
        self.rebuild_annotations();
        self.comment_selected = self
            .comment_selected
            .min(self.comments.len().saturating_sub(1));
        self.prepare_reader_metadata();
        let anchor = if self.content_source == crate::model::ContentSource::Full
        {
            self.document
                .as_ref()
                .map_or(0, |document| document.progress)
        } else {
            data.progress
        };
        // Terminal geometry arrives after construction. Retain the source
        // anchor until the first real layout instead of clamping it away.
        let anchor = self
            .reading_article
            .and_then(|id| {
                self.progress_memory
                    .borrow()
                    .get(&id, self.content_source, self.content_revision)
                    .copied()
            })
            .map_or(anchor, |progress| progress.anchor);
        self.pending_anchor = Some(anchor);
        self.scroll = 0;
        self.reader_cursor = 0;
        self.reader_column = 0;
        self.reader_goal = None;
        self.selection = None;
        self.horizontal = 0;
        self.wrap_reader();
        self.restore_anchor(anchor);
        self.clamp_scroll();
        Ok(())
    }

    fn prepare_reader_metadata(&mut self) {
        self.reader_tools.index =
            crate::reader::index::DocumentIndex::parse(&self.body);
        self.body_kind = content::body_kind(&self.body);
        self.article_warning =
            if self.content_source == crate::model::ContentSource::Full {
                crate::source::article::rules::warnings(&self.body).join(" · ")
            } else {
                String::new()
            };
        self.reading_minutes = if self.body_kind == BodyKind::Text {
            self.body.split_whitespace().count().div_ceil(220)
        } else {
            0
        };
    }

    pub(super) fn clear_current(&mut self) {
        self.pending_reflow = false;
        self.reading_article = None;
        self.displayed_article = None;
        self.pending_anchor = None;
        self.selection = None;
        self.body.clear();
        self.body_kind = BodyKind::Missing;
        self.reading_minutes = 0;
        self.comments.clear();
        self.annotations.clear();
        self.document = None;
        self.article_warning.clear();
        self.content_source = crate::model::ContentSource::Feed;
        self.content_revision = 0;
        self.reader_tools.index =
            crate::reader::index::DocumentIndex::default();
        self.wrap_reader();
    }

    pub(super) fn wrap_reader(&mut self) {
        let cursor = self.cursor_offset();
        let previous_article_end = self.article_text_end;
        let previous = std::mem::take(&mut self.reader_text);
        let body = if self.body_kind == BodyKind::Missing {
            ""
        } else {
            &self.body
        };
        let mut rendered = if self.view == View::Annotated {
            crate::reader::Rendered::default()
        } else if self.immediate_layout {
            self.renderer.render_now(body, self.reader_width)
        } else {
            self.renderer.render(body, self.reader_width)
        };
        self.rebuild_note_marks(&rendered);
        self.paint_highlights(&mut rendered);
        self.images.slots = rendered.images;
        self.resolve_image_urls();
        self.reader_styled = rendered.lines;
        self.line_ranges = rendered.ranges;
        self.code_lines = rendered.code;
        self.table_rows = rendered.table_rows;
        self.reader_text = rendered.text;
        self.reader_ranges = rendered.text_ranges;
        self.reader_mappings = rendered.mappings;
        self.links = self.merge_article_links(rendered.links);
        self.article_text_end = self.reader_text.len();
        if self.view != View::Annotated {
            self.append_feed_notice();
        }
        self.append_comments();
        self.refresh_reader_search();
        self.cache_reader_rows();
        self.restore_reflow_position(&previous, previous_article_end, cursor);
    }

    fn cache_reader_rows(&mut self) {
        self.reader_lines =
            self.reader_styled.iter().map(ToString::to_string).collect();
        self.reader_columns = self
            .reader_lines
            .iter()
            .zip(&self.reader_styled)
            .zip(&self.code_lines)
            .map(|((text, line), code)| {
                (*code || text.contains('\t'))
                    .then(|| crate::reader::viewport::RowIndex::new(text, line))
            })
            .collect();
    }

    fn restore_reflow_position(
        &mut self,
        previous: &str,
        previous_article_end: usize,
        cursor: usize,
    ) {
        let unchanged = previous == self.reader_text;
        if !unchanged {
            self.vim.words =
                crate::reader::motion::WordIndex::new(&self.reader_text);
        }
        // Private comments currently include display wraps in readable text.
        // Their reflow must not invalidate a selection of unchanged publisher
        // text. Selections reaching those comments still require exact equality.
        let publisher_unchanged = cursor < self.article_text_end
            && self
                .selection
                .is_none_or(|anchor| anchor < self.article_text_end)
            && previous.get(..previous_article_end)
                == self.reader_text.get(..self.article_text_end);
        if unchanged || publisher_unchanged {
            self.place_cursor(cursor);
            return;
        }
        self.selection = None;
        self.clamp_cursor();
    }

    fn append_feed_notice(&mut self) {
        let message = match self.body_kind {
            BodyKind::Text => return,
            BodyKind::ReferencesOnly => {
                "This feed supplied links or media without separate article text."
            }
            BodyKind::Missing => "This feed supplied no readable article text.",
        };
        let Some(article) = self.current() else {
            return;
        };
        let action = if article.url.is_empty() {
            "No original article URL was provided."
        } else {
            "o  Open the original article in your browser."
        };
        self.append_reader_line("", Style::default());
        let style = Style::new().fg(Color::Rgb(164, 176, 166));
        for line in textwrap::wrap(message, self.reader_width) {
            self.append_reader_line(&line, style);
        }
        self.append_reader_line("", Style::default());
        for line in textwrap::wrap(action, self.reader_width) {
            self.append_reader_line(&line, style);
        }
        if self
            .current()
            .is_some_and(|article| !article.url.is_empty())
        {
            self.append_reader_line("", Style::default());
            for line in textwrap::wrap(
                ":fetch  Retrieve the full article here.",
                self.reader_width,
            ) {
                self.append_reader_line(&line, style);
            }
        }
    }

    fn append_comments(&mut self) {
        if self.comments.is_empty() {
            return;
        }
        self.append_reader_line("", Style::default());
        self.append_reader_line(
            "PRIVATE COMMENTS",
            Style::new().fg(Color::Rgb(225, 179, 109)),
        );
        for (index, comment) in self.comments.clone().into_iter().enumerate() {
            self.append_reader_line("", Style::default());
            let start = self.reader_text.len();
            self.append_reader_line(
                &format!(
                    "[{}] {}",
                    index + 1,
                    chrono::DateTime::from_timestamp(comment.created, 0)
                        .map_or_else(String::new, |date| date
                            .format("%b %d")
                            .to_string())
                ),
                Style::new().fg(Color::Rgb(225, 179, 109)),
            );
            self.append_quote(&comment);
            for line in textwrap::wrap(&comment.body, self.reader_width) {
                self.append_reader_line(&line, Style::default());
            }
            self.note_marks
                .comments
                .push((start..self.reader_text.len(), comment.id));
        }
    }

    fn append_quote(&mut self, comment: &crate::model::Comment) {
        let quote = &comment.quote;
        if quote.is_empty() {
            return;
        }
        for line in
            textwrap::wrap(quote, self.reader_width.saturating_sub(2).max(1))
        {
            self.append_reader_line(
                &format!("│ {line}"),
                Style::new().fg(Color::Rgb(164, 176, 166)),
            );
        }
        if comment.source != self.content_source {
            self.append_reader_line(
                &format!("[Quote from {} content.]", comment.source.as_str()),
                Style::default(),
            );
        } else if comment.source == crate::model::ContentSource::Full
            && comment.source_revision != self.content_revision
        {
            self.append_reader_line(
                &format!(
                    "[Quote from Full revision {}; current revision {}.]",
                    comment.source_revision, self.content_revision
                ),
                Style::default(),
            );
        } else if !self.body.contains(quote) {
            self.append_reader_line(
                "[Passage changed at the publisher; original quote preserved.]",
                Style::default(),
            );
        }
    }

    fn append_reader_line(&mut self, text: &str, style: Style) {
        let text = crate::content::clean(text);
        let start = self.reader_text.len();
        self.reader_text.push_str(&text);
        self.reader_ranges.push(start..self.reader_text.len());
        self.reader_text.push('\n');
        self.reader_styled.push(Line::styled(text, style));
        self.line_ranges.push(self.body.len()..self.body.len());
        self.code_lines.push(false);
    }

    pub(super) fn restore_anchor(&mut self, anchor: usize) {
        if self.view == View::Annotated {
            return;
        }
        self.scroll = self
            .line_ranges
            .iter()
            .position(|range| range.end > anchor)
            .unwrap_or(0);
        self.reader_cursor = self.scroll;
        self.reader_column = 0;
    }

    pub(super) fn row_column(&self, row: usize, byte: usize) -> usize {
        let text = self.reader_lines.get(row).map_or("", String::as_str);
        self.reader_columns
            .get(row)
            .and_then(Option::as_ref)
            .map_or_else(
                || crate::reader::cursor::column(text, byte),
                |index| index.column(text, byte),
            )
    }

    pub(super) fn row_byte_at(&self, row: usize, goal: usize) -> usize {
        let text = self.reader_lines.get(row).map_or("", String::as_str);
        self.reader_columns
            .get(row)
            .and_then(Option::as_ref)
            .map_or_else(
                || crate::reader::cursor::byte_at(text, goal),
                |index| index.byte_at(text, goal),
            )
    }

    pub(super) fn clamp_scroll(&mut self) {
        self.scroll = self
            .scroll
            .min(self.reader_lines.len().saturating_sub(self.reader_height));
        self.clamp_cursor();
    }

    pub(super) fn move_reader(&mut self, forward: bool, amount: usize) {
        self.pending_anchor = None;
        let goal = self.reader_goal.unwrap_or_else(|| {
            self.row_column(self.reader_cursor, self.reader_column)
        });
        self.reader_goal = Some(goal);
        self.reader_cursor = if forward {
            self.reader_cursor
                .saturating_add(amount)
                .min(self.reader_lines.len().saturating_sub(1))
        } else {
            self.reader_cursor.saturating_sub(amount)
        };
        self.reader_column = self.row_byte_at(self.reader_cursor, goal);
        self.ensure_cursor_visible();
    }

    pub(super) fn save_reading_position(&self) -> Result<()> {
        let Some(article) = self
            .current()
            .filter(|article| Some(article.id) == self.reading_article)
        else {
            return Ok(());
        };
        if self.view == View::Annotated || self.reader_text.is_empty() {
            return Ok(());
        }
        let anchor = self.pending_anchor.unwrap_or_else(|| {
            self.line_ranges
                .get(self.scroll)
                .map_or(0, |range| range.start)
        });
        let progress = super::session_writer::Progress {
            article: article.id,
            source: self.content_source,
            revision: self.content_revision,
            anchor,
        };
        if self.progress_memory.borrow().get(
            &article.id,
            self.content_source,
            self.content_revision,
        ) == Some(&progress)
        {
            return Ok(());
        }
        if let Some(writer) = &self.session_writer {
            writer.progress(progress)?;
        } else {
            progress.save(&self.store)?;
        }
        self.progress_memory.borrow_mut().remember(progress);
        Ok(())
    }

    /// Save the layout, active destination and reading position before exit.
    ///
    /// # Errors
    /// Returns storage failures; the terminal runner still restores its state.
    pub fn save_session(&mut self) -> Result<()> {
        self.save_reading_position()?;
        self.workspace.article = self.current().map(|article| article.id);
        self.workspace.view = self.view.clone();
        self.workspace.page_offset = self.page_offset;
        self.persist_preferences()
    }

    pub(super) fn persist_preferences(&mut self) -> Result<()> {
        if self.persist_notebook_layout()? {
            return Ok(());
        }
        self.layouts
            .observe(crate::model::PanelLayout::capture(&self.workspace));
        if let Some(writer) = &self.session_writer {
            return writer.layout(&self.workspace, &self.layouts);
        }
        self.store.save_layouts(&self.workspace, &self.layouts)
    }

    /// Persist the session boundary only when leaving the terminal application.
    ///
    /// # Errors
    /// Returns local storage errors while preserving terminal cleanup.
    pub fn end_session(&mut self) -> Result<()> {
        self.flush_recommendations()?;
        self.checkpoint_draft()?;
        // Global Ctrl-c can leave this modal without its explicit confirmation.
        if let super::Mode::Settings(draft) = &self.mode {
            self.workspace = draft.before.clone();
            self.mode = super::Mode::Normal;
        }
        self.workspace.last_visit = chrono::Utc::now().timestamp();
        let save = self.save_session();
        let flush = self.flush_session();
        save.and(flush)
    }
}
