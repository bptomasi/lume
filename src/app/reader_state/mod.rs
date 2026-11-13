//! Publisher document, projection and viewport invariants owned by one reader.
//!
//! Storage, application modes and background navigation remain coordinators.
//! Presentation borrows the cached projection and cannot mutate this state.

mod annotations;
mod auto_read;
mod document;
mod formulas;
mod horizontal;
mod motions;
mod note_marks;
mod projection;
mod resize;
mod scroll;
mod selection;
mod tools;
mod transitions;
mod vertical;
mod view;

pub(in crate::app) use formulas::FormulaMotion;
pub(in crate::app) use projection::ReaderSnapshot;
pub use tools::ReaderTools;
pub(in crate::app) use transitions::RememberedProgress;
pub use view::ReaderView;

use super::{AnnotationEntry, Comment};
use crate::model::Article;
use anyhow::Result;
use horizontal::Horizontal;

#[derive(Clone, Copy)]
pub(super) struct RenderContext {
    pub(super) annotated: bool,
    pub(super) immediate: bool,
    pub(super) justify: bool,
    pub(super) options: crate::reader::RenderOptions,
}

/// One opened publisher source with an atomic projection and cursor lifecycle.
/// Application selection may move while this reader retains its old identity.
pub(super) struct ReaderState {
    publisher: PublisherDocument,
    projection: Projection,
    viewport: Viewport,
    notes: AnnotationState,
    auto_read: auto_read::AutoRead,
    motion_status: Option<String>,
    synchronous_loading: bool,
    context: RenderContext,
    renderer: crate::reader::LiveRenderer,
}

impl ReaderState {
    pub(in crate::app) fn synchronous_loading(&self) -> bool {
        self.synchronous_loading
    }

    pub(in crate::app) fn set_synchronous_loading(&mut self, loading: bool) {
        self.synchronous_loading = loading;
    }

    pub(in crate::app) fn new() -> Result<Self> {
        Ok(Self {
            publisher: PublisherDocument::default(),
            projection: Projection::default(),
            viewport: Viewport::default(),
            notes: AnnotationState::default(),
            auto_read: auto_read::AutoRead::default(),
            motion_status: None,
            synchronous_loading: false,
            context: RenderContext {
                annotated: false,
                immediate: false,
                justify: false,
                options: crate::reader::RenderOptions::default(),
            },
            renderer: crate::reader::LiveRenderer::start()?,
        })
    }
}

// Publisher bytes and independent cache identity survive selection changes.
struct PublisherDocument {
    body: String,
    kind: crate::content::BodyKind,
    source: crate::model::ContentSource,
    revision: i64,
    cache: Option<crate::model::CachedDocument>,
    origin: crate::model::FeedOrigin,
    warning: String,
    fetch_policy: crate::model::FetchPolicy,
    feed_kind: crate::content::BodyKind,
    feed_links: Vec<(String, String)>,
    minutes: usize,
    article: Option<Article>,
}

impl Default for PublisherDocument {
    fn default() -> Self {
        Self {
            body: String::new(),
            kind: crate::content::BodyKind::Missing,
            source: crate::model::ContentSource::Feed,
            revision: 0,
            cache: None,
            origin: crate::model::FeedOrigin::Feed,
            warning: String::new(),
            fetch_policy: crate::model::FetchPolicy::Missing,
            feed_kind: crate::content::BodyKind::Missing,
            feed_links: Vec::new(),
            minutes: 0,
            article: None,
        }
    }
}

// These parallel row/source caches are installed and invalidated together.
#[derive(Default)]
struct Projection {
    words: crate::reader::motion::WordIndex,
    images: Vec<crate::reader::images::ImageSlot>,
    lines: Vec<String>,
    text: String,
    tools: ReaderTools,
    ranges: Vec<std::ops::Range<usize>>,
    mappings: Vec<crate::reader::SourceMapping>,
    article_end: usize,
    source_ranges: Vec<std::ops::Range<usize>>,
    columns: Vec<Option<crate::reader::viewport::RowIndex>>,
    styled: Vec<ratatui::text::Line<'static>>,
    links: Vec<(String, String)>,
    horizontal_groups: Vec<Option<usize>>,
    justifiable: Vec<bool>,
    insets: Vec<usize>,
    code: Vec<bool>,
    tables: Vec<std::ops::Range<usize>>,
}

// Positions refer only to the current projection, never the selected article.
struct Viewport {
    first: usize,
    width: usize,
    height: usize,
    pending_anchor: Option<usize>,
    selection: Option<usize>,
    row: usize,
    byte: usize,
    goal: Option<usize>,
    linewise: bool,
    horizontal: usize,
    horizontal_state: Horizontal,
    pending_reflow: bool,
    formula_position: Option<formulas::FormulaPosition>,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            first: 0,
            width: 72,
            height: 20,
            pending_anchor: None,
            selection: None,
            row: 0,
            byte: 0,
            goal: None,
            linewise: false,
            horizontal: 0,
            horizontal_state: Horizontal::default(),
            pending_reflow: false,
            formula_position: None,
        }
    }
}

// Private notes remain outside publisher text and its source mappings.
#[derive(Default)]
struct AnnotationState {
    comments: Vec<Comment>,
    entries: Vec<AnnotationEntry>,
    marks: note_marks::NoteMarks,
    selected: usize,
}

#[cfg(test)]
mod tests;
