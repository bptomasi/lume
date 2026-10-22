//! Replaceable reads prepare bounded note pages and source context off input.

use std::path::Path;

use anyhow::Result;

use crate::{
    latest::Latest,
    model::{AnnotationSnapshot, NoteCollection, NotePage, NoteSummary},
    store::Store,
};

/// Cached note and source excerpt, prepared without work in the paint loop.
pub struct NoteContext {
    /// Complete note with independent study metadata.
    pub note: AnnotationSnapshot,
    /// Source headline.
    pub title: String,
    /// Source URL.
    pub url: String,
    /// Prepared note Markdown.
    pub lines: Vec<ratatui::text::Line<'static>>,
    /// Prepared excerpt from the owning source.
    pub source_lines: Vec<ratatui::text::Line<'static>>,
    /// At most 128 incoming references.
    pub backlinks: Vec<NoteSummary>,
    /// At most 128 outgoing references.
    pub references: Vec<NoteSummary>,
    /// Verified source position or an explicit unavailable/unplaced label.
    pub placement: String,
}

pub(super) struct ListRequest {
    pub(super) text: String,
    pub(super) article: Option<i64>,
    pub(super) offset: usize,
    pub(super) recent_generation: u64,
}

pub(super) struct Listing {
    pub(super) page: NotePage,
    pub(super) offset: usize,
    pub(super) recent_generation: u64,
    pub(super) collections: Vec<NoteCollection>,
    pub(super) recent: Vec<String>,
}

pub(super) struct Readers {
    pub(super) list: Latest<ListRequest, Listing>,
    pub(super) context: Latest<(i64, usize), NoteContext>,
}

impl Readers {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let list_store = Store::open(path)?;
        let context_store = Store::open(path)?;
        Ok(Self {
            list: Latest::start("lume-note-search", move |request| {
                listing(&list_store, request)
            })?,
            context: Latest::start("lume-note-context", move |(id, width)| {
                context(&context_store, id, width)
            })?,
        })
    }
}

fn listing(store: &Store, request: ListRequest) -> Result<Listing> {
    let mut offset = request.offset;
    let mut page =
        store.search_notes(&request.text, request.article, offset, 100)?;
    if page.notes.is_empty() && page.total > 0 {
        offset = (page.total - 1) / 100 * 100;
        page =
            store.search_notes(&request.text, request.article, offset, 100)?;
    }
    Ok(Listing {
        page,
        offset,
        recent_generation: request.recent_generation,
        collections: store.note_collections()?,
        recent: store.note_search_history()?,
    })
}

fn context(store: &Store, id: i64, width: usize) -> Result<NoteContext> {
    let detail = store.note_detail(id)?;
    let excerpt = source_excerpt(&detail);
    let lines = crate::reader::render(&detail.note.comment.body, width).lines;
    let source_lines = render_excerpt(&excerpt, width);
    Ok(NoteContext {
        title: detail.summary.title,
        url: detail.url,
        backlinks: detail.backlinks,
        references: detail.references,
        note: detail.note,
        lines,
        source_lines,
        placement: excerpt.placement,
    })
}

struct Excerpt {
    placement: String,
    text: String,
    quote: Option<std::ops::Range<usize>>,
}

fn source_excerpt(detail: &crate::model::NoteDetail) -> Excerpt {
    let comment = &detail.note.comment;
    if !detail.source_available {
        return Excerpt {
            placement: "Original source unavailable · quotation retained"
                .into(),
            text: comment.quote.clone(),
            quote: None,
        };
    }
    if comment.quote.is_empty() {
        return Excerpt {
            placement: "General note · article introduction".into(),
            text: excerpt(&detail.body, 0, 2048),
            quote: None,
        };
    }
    let Some(range) = super::super::passages::locate_quote(
        &detail.body,
        &comment.quote,
        if comment.source_revision == detail.source_revision {
            comment.anchor
        } else {
            detail.body.len()
        },
    ) else {
        return Excerpt {
            placement: "Unplaced quote · select source and :note-reanchor"
                .into(),
            text: comment.quote.clone(),
            quote: None,
        };
    };
    located_excerpt(detail, range)
}

fn located_excerpt(
    detail: &crate::model::NoteDetail,
    range: std::ops::Range<usize>,
) -> Excerpt {
    let start = detail
        .body
        .floor_char_boundary(range.start.saturating_sub(768));
    let end = (range.end + 768).min(detail.body.len());
    Excerpt {
        placement: format!(
            "{} source · {}%",
            detail.note.comment.source.as_str(),
            range.start.saturating_mul(100) / detail.body.len().max(1)
        ),
        text: excerpt(&detail.body, start, end),
        quote: Some(range.start - start..range.end - start),
    }
}

fn render_excerpt(
    excerpt: &Excerpt,
    width: usize,
) -> Vec<ratatui::text::Line<'static>> {
    let mut rendered = crate::reader::render(&excerpt.text, width);
    if let Some(source) = &excerpt.quote {
        let marks =
            crate::reader::to_display(&rendered.mappings, source.clone());
        let style = ratatui::style::Style::new()
            .bg(ratatui::style::Color::Rgb(65, 53, 32));
        for (line, row) in rendered.lines.iter_mut().zip(&rendered.text_ranges)
        {
            for mark in &marks {
                let start = row.start.max(mark.start);
                let end = row.end.min(mark.end);
                if start < end {
                    crate::reader::style_range(
                        line,
                        start - row.start..end - row.start,
                        style,
                    );
                }
            }
        }
    }
    for (line, inset) in rendered.lines.iter_mut().zip(&rendered.insets) {
        if *inset > 0 {
            line.spans
                .insert(0, ratatui::text::Span::raw(" ".repeat(*inset)));
        }
    }
    rendered.lines
}

fn excerpt(body: &str, start: usize, end: usize) -> String {
    let start = body.floor_char_boundary(start.min(body.len()));
    let end = body.floor_char_boundary(end.min(body.len()));
    body[start..end].to_owned()
}
