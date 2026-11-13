//! Borrowed cached presentation. Construction performs no parsing or I/O.
use super::ReaderState;
use crate::{
    app::{AnnotationEntry, ReaderTools},
    model::{Article, Comment},
};

/// Cached publisher source, rendered rows and current viewport.
///
/// All references are borrowed from the reader. No storage, navigation or
/// worker capabilities are carried into presentation.
#[derive(Clone, Copy)]
pub struct ReaderView<'a> {
    horizontal_blocks: super::horizontal::HorizontalView<'a>,
    note_markers: &'a std::collections::BTreeMap<usize, Vec<i64>>,
    note_hover: Option<(i64, ratatui::layout::Position)>,
    /// Non-overlapping publisher images and formulas in display-row order.
    pub image_slots: &'a [crate::reader::images::ImageSlot],
    /// Original publisher bytes for the source actually displayed.
    pub body: &'a str,
    /// Availability of readable text in the displayed source.
    pub body_kind: crate::content::BodyKind,
    /// Displayed Feed or Full source, independent of pending selection.
    pub content_source: crate::model::ContentSource,
    /// Full-body revision; zero for Feed content.
    pub content_revision: i64,
    /// Independent Full cache and most recent fetch outcome, if any.
    pub document: Option<&'a crate::model::CachedDocument>,
    /// Parser-recorded Feed, Summary or Missing origin.
    pub feed_origin: crate::model::FeedOrigin,
    /// Extraction warning outside quotation and source mappings.
    pub article_warning: &'a str,
    /// Automatic extraction policy of the displayed subscription.
    pub fetch_policy: crate::model::FetchPolicy,
    /// Reading-time estimate at 220 words per minute.
    pub reading_minutes: usize,
    /// Private comments in durable creation order.
    pub comments: &'a [Comment],
    /// Article-local annotation rows in resolved source order.
    pub annotations: &'a [AnnotationEntry],
    /// Selected index into `comments` for edit/delete actions.
    pub comment_selected: usize,
    /// Plain display rows, cached when the projection changes.
    pub reader_lines: &'a [String],
    /// First visible display row, clamped to a full final page.
    pub scroll: usize,
    /// Reader width in terminal columns, always at least one.
    pub reader_width: usize,
    /// Visible body rows, always at least one.
    pub reader_height: usize,
    /// Readable UTF-8 byte anchor of the active visual selection.
    pub selection: Option<usize>,
    /// Cursor display-row index, clamped to available rows.
    pub reader_cursor: usize,
    /// Grapheme-aligned byte offset within the cursor display row.
    pub reader_column: usize,
    /// Whether visual selection includes complete display rows.
    pub linewise: bool,
    /// Selectable UTF-8 text, including generated notices and private comments.
    pub reader_text: &'a str,
    /// Width-independent structure, literal search and cached hits.
    pub reader_tools: &'a ReaderTools,
    /// Metadata for the displayed source, retained during navigation.
    pub displayed_article: Option<&'a Article>,
    /// Readable-text UTF-8 byte range for each display row.
    pub reader_ranges: &'a [std::ops::Range<usize>],
    /// Parser correspondence between readable and publisher bytes.
    pub reader_mappings: &'a [crate::reader::SourceMapping],
    /// Publisher-byte ranges per row; generated rows use an empty source tail.
    pub line_ranges: &'a [std::ops::Range<usize>],
    /// Optional cached terminal-column projection for each display row.
    pub(crate) reader_columns:
        &'a [Option<crate::reader::viewport::RowIndex>],
    /// Styled display rows; drawing never reparses or highlights.
    pub reader_styled: &'a [ratatui::text::Line<'static>],
    /// HTTP(S) references in encounter order, including feed references.
    pub links: &'a [(String, String)],
    /// Active code/table viewport offset in terminal columns.
    pub horizontal: usize,
    /// Rows preserving source columns, including code and tables.
    pub code_lines: &'a [bool],
    /// Display-row ranges of tables, which retain the panel background.
    pub table_rows: &'a [std::ops::Range<usize>],
}

impl ReaderState {
    pub(in crate::app) fn view(&self) -> ReaderView<'_> {
        ReaderView {
            horizontal_blocks: self.horizontal_view(),
            note_markers: &self.notes.marks.rows,
            note_hover: self.notes.marks.hover,
            image_slots: &self.projection.images,
            body: &self.publisher.body,
            body_kind: self.publisher.kind,
            content_source: self.publisher.source,
            content_revision: self.publisher.revision,
            document: self.publisher.cache.as_ref(),
            feed_origin: self.publisher.origin,
            article_warning: &self.publisher.warning,
            fetch_policy: self.publisher.fetch_policy,
            reading_minutes: self.publisher.minutes,
            comments: &self.notes.comments,
            annotations: &self.notes.entries,
            comment_selected: self.notes.selected,
            reader_lines: &self.projection.lines,
            scroll: self.viewport.first,
            reader_width: self.viewport.width,
            reader_height: self.viewport.height,
            selection: self.viewport.selection,
            reader_cursor: self.viewport.row,
            reader_column: self.viewport.byte,
            linewise: self.viewport.linewise,
            reader_text: &self.projection.text,
            reader_tools: &self.projection.tools,
            displayed_article: self.publisher.article.as_ref(),
            reader_ranges: &self.projection.ranges,
            reader_mappings: &self.projection.mappings,
            line_ranges: &self.projection.source_ranges,
            reader_columns: &self.projection.columns,
            reader_styled: &self.projection.styled,
            links: &self.projection.links,
            horizontal: self.viewport.horizontal,
            code_lines: &self.projection.code,
            table_rows: &self.projection.tables,
        }
    }
}

impl<'a> ReaderView<'a> {
    /// Current readable-text byte offset, independent of wrapping.
    pub fn cursor_offset(&self) -> usize {
        self.reader_ranges
            .get(self.reader_cursor)
            .map_or(0, |range| {
                range.start + self.reader_column.min(range.len())
            })
    }

    /// Inclusive visual selection as a grapheme-safe half-open byte range.
    pub fn selected_range(&self) -> Option<std::ops::Range<usize>> {
        super::selection::selected_range(
            self.reader_text,
            self.reader_ranges,
            self.selection,
            self.cursor_offset(),
            self.linewise,
        )
    }

    /// Horizontal offset for this row's code/table block, in terminal columns.
    pub fn horizontal_for_row(&self, row: usize) -> usize {
        self.horizontal_blocks.for_row(row)
    }

    /// Annotation identities beginning on a display row.
    pub fn note_markers(&self, row: usize) -> &'a [i64] {
        self.note_markers.get(&row).map_or(&[], Vec::as_slice)
    }

    /// Stable hover target and popup origin, without parsing or storage reads.
    pub fn hovered_note(&self) -> Option<(i64, ratatui::layout::Position)> {
        self.note_hover
    }

    /// Selected position in resolved annotation order.
    pub fn annotation_selected(&self) -> Option<usize> {
        if self.annotations.is_empty() {
            return None;
        }
        Some(
            self.annotations
                .iter()
                .position(|entry| entry.comment_index == self.comment_selected)
                .unwrap_or(0),
        )
    }
}
