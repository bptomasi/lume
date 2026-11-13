//! Source installation and cached projection lifecycle.
use super::{ReaderState, RenderContext};
use crate::app::ReaderIdentity;

/// In-memory viewport, reusable only while the displayed source stays intact.
pub(in crate::app) struct ReaderSnapshot {
    identity: Option<ReaderIdentity>,
    comment: Option<i64>,
    body: String,
    notes: Option<String>,
    cursor: usize,
    selection: Option<usize>,
    linewise: bool,
    scroll: usize,
    horizontal: usize,
    horizontal_state: super::Horizontal,
    pending_anchor: Option<usize>,
}

use crate::content::{self, BodyKind};
use ratatui::{
    style::{Color, Style},
    text::Line,
};

impl ReaderState {
    pub(super) fn restore_ready_viewport(
        &mut self,
        anchor: usize,
        cursor: usize,
    ) {
        let reflow = std::mem::take(&mut self.viewport.pending_reflow);
        if self.viewport.pending_anchor.take().is_some() && !reflow {
            self.restore_anchor(anchor);
            return;
        }
        self.viewport.first = self
            .projection
            .source_ranges
            .iter()
            .position(|range| range.end > anchor)
            .unwrap_or(0);
        self.place_cursor(cursor);
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn reading_article(&self) -> Option<i64> {
        self.publisher.article.as_ref().map(|article| article.id)
    }

    pub(in crate::app) fn reader_identity(&self) -> Option<ReaderIdentity> {
        Some(ReaderIdentity {
            article: self.reading_article()?,
            source: self.publisher.source,
            revision: self.publisher.revision,
        })
    }

    pub(in crate::app) fn reader_pending(&self) -> bool {
        self.renderer.is_pending()
    }

    pub(in crate::app) fn capture_reader(&self) -> ReaderSnapshot {
        let cursor = self.cursor_offset();
        ReaderSnapshot {
            identity: self.reader_identity(),
            comment: self
                .notes
                .comments
                .get(self.notes.selected)
                .map(|note| note.id),
            body: self.publisher.body.clone(),
            notes: (cursor >= self.projection.article_end
                || self.viewport.selection.is_some_and(|anchor| {
                    anchor >= self.projection.article_end
                }))
            .then(|| self.projection.text.clone()),
            cursor,
            selection: self.viewport.selection,
            linewise: self.viewport.linewise,
            scroll: self.viewport.first,
            horizontal: self.viewport.horizontal,
            horizontal_state: self.viewport.horizontal_state.clone(),
            pending_anchor: self.viewport.pending_anchor,
        }
    }

    pub(in crate::app) fn restore_reader(
        &mut self,
        snapshot: ReaderSnapshot,
        selected: Option<i64>,
    ) {
        let Some(identity) = snapshot.identity else {
            return;
        };
        if selected != Some(identity.article)
            || self.reading_article() != Some(identity.article)
        {
            return;
        }
        if let Some(position) = self
            .notes
            .comments
            .iter()
            .position(|note| Some(note.id) == snapshot.comment)
        {
            self.notes.selected = position;
        }
        if self.reader_identity() != Some(identity)
            || self.publisher.body != snapshot.body
            || snapshot
                .notes
                .is_some_and(|notes| notes != self.projection.text)
        {
            return;
        }
        self.viewport.selection = snapshot.selection;
        self.viewport.linewise = snapshot.linewise;
        self.viewport.pending_anchor = snapshot.pending_anchor;
        self.viewport.first = snapshot.scroll;
        self.viewport.horizontal = snapshot.horizontal;
        self.viewport.horizontal_state = snapshot.horizontal_state;
        self.place_cursor(snapshot.cursor);
        self.ensure_cursor_visible();
    }

    pub(super) fn prepare_reader_metadata(&mut self) {
        self.projection.tools.index =
            crate::reader::index::DocumentIndex::parse_with_options(
                &self.publisher.body,
                self.context.options,
            );
        self.publisher.kind = content::body_kind(&self.publisher.body);
        self.publisher.warning =
            if self.publisher.source == crate::model::ContentSource::Full {
                crate::source::article::rules::warnings(&self.publisher.body)
                    .join(" · ")
            } else {
                self.publisher
                    .cache
                    .as_ref()
                    .filter(|document| {
                        self.publisher
                            .article
                            .as_ref()
                            .is_some_and(|article| article.url == document.url)
                    })
                    .and_then(|document| {
                        crate::content::extraction::rejection(&document.body)
                    })
                    .map(|reason| {
                        format!("{reason} · showing feed · o opens original")
                    })
                    .unwrap_or_default()
            };
        self.publisher.minutes = if self.publisher.kind == BodyKind::Text {
            self.publisher.body.split_whitespace().count().div_ceil(220)
        } else {
            0
        };
    }

    pub(in crate::app) fn clear_current(&mut self, context: RenderContext) {
        self.viewport = super::Viewport {
            width: self.viewport.width,
            height: self.viewport.height,
            ..super::Viewport::default()
        };
        self.publisher = super::PublisherDocument::default();
        self.notes = super::AnnotationState::default();
        self.motion_status = None;
        self.projection.tools.index =
            crate::reader::index::DocumentIndex::default();
        self.wrap_reader(context);
    }

    pub(in crate::app) fn wrap_reader(&mut self, context: RenderContext) {
        self.context = context;
        self.renderer.set_render_options(self.context.options);
        let cursor = self.cursor_offset();
        let previous_article_end = self.projection.article_end;
        let previous = std::mem::take(&mut self.projection.text);
        let body = if self.publisher.kind == BodyKind::Missing {
            ""
        } else {
            &self.publisher.body
        };
        let mut rendered = if self.context.annotated {
            crate::reader::Rendered::default()
        } else if self.context.immediate {
            self.renderer.render_now(body, self.viewport.width)
        } else {
            self.renderer.render(body, self.viewport.width)
        };
        self.rebuild_note_marks(&rendered);
        self.paint_highlights(&mut rendered);
        self.projection.images = rendered.images;
        self.resolve_image_urls();
        self.projection.styled = rendered.lines;
        self.projection.source_ranges = rendered.ranges;
        self.projection.code = rendered.code;
        self.projection.horizontal_groups = rendered.horizontal_groups;
        self.projection.justifiable = rendered.justifiable;
        self.projection.insets = rendered.insets;
        self.projection.tables = rendered.table_rows;
        self.projection.text = rendered.text;
        self.projection.ranges = rendered.text_ranges;
        self.projection.mappings = rendered.mappings;
        self.projection.links = self.merge_article_links(rendered.links);
        self.projection.article_end = self.projection.text.len();
        if !self.context.annotated {
            self.append_feed_notice();
        }
        self.append_comments();
        self.refresh_reader_search();
        self.cache_reader_rows();
        self.restore_reflow_position(&previous, previous_article_end, cursor);
    }

    pub(super) fn cache_reader_rows(&mut self) {
        self.projection.lines = self
            .projection
            .styled
            .iter()
            .map(ToString::to_string)
            .collect();
        self.projection.columns = self
            .projection
            .lines
            .iter()
            .zip(&self.projection.styled)
            .zip(&self.projection.code)
            .enumerate()
            .map(|(row, ((text, line), code))| {
                let inset =
                    self.projection.insets.get(row).copied().unwrap_or(0);
                if inset > 0 {
                    return Some(crate::reader::viewport::RowIndex::inset(
                        text, line, inset,
                    ));
                }
                if self.context.justify
                    && self.projection.justifiable.get(row) == Some(&true)
                {
                    return Some(crate::reader::viewport::RowIndex::justified(
                        text,
                        line,
                        self.viewport.width,
                    ));
                }
                (*code || text.contains('\t'))
                    .then(|| crate::reader::viewport::RowIndex::new(text, line))
            })
            .collect();
    }

    pub(super) fn restore_reflow_position(
        &mut self,
        previous: &str,
        previous_article_end: usize,
        cursor: usize,
    ) {
        let unchanged = previous == self.projection.text;
        if !unchanged {
            self.projection.words =
                crate::reader::motion::WordIndex::new(&self.projection.text);
        }
        // Private comments currently include display wraps in readable text.
        // Their reflow must not invalidate a selection of unchanged publisher
        // text. Selections reaching those comments still require exact equality.
        let publisher_unchanged = previous.get(..previous_article_end)
            == self.projection.text.get(..self.projection.article_end);
        if !publisher_unchanged {
            self.auto_read_text_changed();
        }
        let publisher_unchanged = publisher_unchanged
            && cursor < self.projection.article_end
            && self
                .viewport
                .selection
                .is_none_or(|anchor| anchor < self.projection.article_end);
        if unchanged || publisher_unchanged {
            self.place_cursor(cursor);
            return;
        }
        self.viewport.selection = None;
        self.clamp_cursor();
    }

    pub(super) fn append_feed_notice(&mut self) {
        let message = match self.publisher.kind {
            BodyKind::Text => return,
            BodyKind::ReferencesOnly => {
                "This feed supplied links or media without separate article text."
            }
            BodyKind::Missing => "This feed supplied no readable article text.",
        };
        let Some(article) = self.publisher.article.as_ref() else {
            return;
        };
        let action = if article.url.is_empty() {
            "No original article URL was provided."
        } else {
            "o  Open the original article in your browser."
        };
        self.append_reader_line("", Style::default());
        let style = Style::new().fg(Color::Rgb(164, 176, 166));
        for line in textwrap::wrap(message, self.viewport.width) {
            self.append_reader_line(&line, style);
        }
        self.append_reader_line("", Style::default());
        for line in textwrap::wrap(action, self.viewport.width) {
            self.append_reader_line(&line, style);
        }
        if self
            .publisher
            .article
            .as_ref()
            .is_some_and(|article| !article.url.is_empty())
        {
            self.append_reader_line("", Style::default());
            for line in textwrap::wrap(
                ":fetch  Retrieve the full article here.",
                self.viewport.width,
            ) {
                self.append_reader_line(&line, style);
            }
        }
    }

    pub(super) fn append_comments(&mut self) {
        if self.notes.comments.is_empty() {
            return;
        }
        self.append_reader_line("", Style::default());
        self.append_reader_line(
            "PRIVATE COMMENTS",
            Style::new().fg(Color::Rgb(225, 179, 109)),
        );
        for (index, comment) in
            self.notes.comments.clone().into_iter().enumerate()
        {
            self.append_reader_line("", Style::default());
            let start = self.projection.text.len();
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
            for line in textwrap::wrap(&comment.body, self.viewport.width) {
                self.append_reader_line(&line, Style::default());
            }
            self.notes
                .marks
                .comments
                .push((start..self.projection.text.len(), comment.id));
        }
    }

    pub(super) fn append_quote(&mut self, comment: &crate::model::Comment) {
        let quote = &comment.quote;
        if quote.is_empty() {
            return;
        }
        for line in
            textwrap::wrap(quote, self.viewport.width.saturating_sub(2).max(1))
        {
            self.append_reader_line(
                &format!("│ {line}"),
                Style::new().fg(Color::Rgb(164, 176, 166)),
            );
        }
        if comment.source != self.publisher.source {
            self.append_reader_line(
                &format!("[Quote from {} content.]", comment.source.as_str()),
                Style::default(),
            );
        } else if comment.source == crate::model::ContentSource::Full
            && comment.source_revision != self.publisher.revision
        {
            self.append_reader_line(
                &format!(
                    "[Quote from Full revision {}; current revision {}.]",
                    comment.source_revision, self.publisher.revision
                ),
                Style::default(),
            );
        } else if !self.publisher.body.contains(quote) {
            self.append_reader_line(
                "[Passage changed at the publisher; original quote preserved.]",
                Style::default(),
            );
        }
    }

    pub(super) fn append_reader_line(&mut self, text: &str, style: Style) {
        let text = crate::content::clean(text);
        let start = self.projection.text.len();
        self.projection.text.push_str(&text);
        self.projection
            .ranges
            .push(start..self.projection.text.len());
        self.projection.text.push('\n');
        self.projection.styled.push(Line::styled(text, style));
        self.projection
            .source_ranges
            .push(self.publisher.body.len()..self.publisher.body.len());
        self.projection.code.push(false);
    }

    pub(in crate::app) fn restore_anchor(&mut self, anchor: usize) {
        if self.context.annotated {
            return;
        }
        self.viewport.first = self
            .projection
            .source_ranges
            .iter()
            .position(|range| range.end > anchor)
            .unwrap_or(0);
        self.viewport.row = self.viewport.first;
        self.viewport.byte = 0;
    }

    pub(super) fn row_column(&self, row: usize, byte: usize) -> usize {
        let text = self.projection.lines.get(row).map_or("", String::as_str);
        self.projection
            .columns
            .get(row)
            .and_then(Option::as_ref)
            .map_or_else(
                || crate::reader::cursor::column(text, byte),
                |index| index.column(text, byte),
            )
    }

    pub(in crate::app) fn row_byte_at(&self, row: usize, goal: usize) -> usize {
        let text = self.projection.lines.get(row).map_or("", String::as_str);
        self.projection
            .columns
            .get(row)
            .and_then(Option::as_ref)
            .map_or_else(
                || crate::reader::cursor::byte_at(text, goal),
                |index| index.byte_at(text, goal),
            )
    }

    pub(in crate::app) fn clamp_scroll(&mut self) {
        self.viewport.first = self.viewport.first.min(
            self.projection
                .lines
                .len()
                .saturating_sub(self.viewport.height),
        );
        self.clamp_cursor();
    }
}

impl ReaderState {
    fn resolve_image_urls(&mut self) {
        let address = self
            .publisher
            .cache
            .as_ref()
            .filter(|_| {
                self.publisher.source == crate::model::ContentSource::Full
            })
            .map(|document| document.final_url.as_str())
            .filter(|url| !url.is_empty())
            .or_else(|| {
                self.publisher
                    .article
                    .as_ref()
                    .map(|article| article.url.as_str())
            });
        let base = address.and_then(|url| crate::urls::feed_url(url).ok());
        crate::app::images::resolve_urls(
            &mut self.projection.images,
            base.as_ref(),
        );
    }
}

impl ReaderState {
    pub(in crate::app) fn reproject_ready(&mut self, context: RenderContext) {
        let cursor = self.cursor_offset();
        let anchor = self.viewport.pending_anchor.unwrap_or_else(|| {
            self.projection
                .source_ranges
                .get(self.viewport.first)
                .map_or(0, |range| range.start)
        });
        let horizontal = self.viewport.horizontal;
        self.wrap_reader(context);
        self.viewport.horizontal = horizontal;
        if !self.source_preparing() && !self.restore_formula_position() {
            self.restore_ready_viewport(anchor, cursor);
        }
        self.clamp_scroll();
    }
}

impl ReaderState {
    pub(in crate::app) fn apply_justification(&mut self, enabled: bool) {
        self.context.justify = enabled;
        self.cache_reader_rows();
    }
}
