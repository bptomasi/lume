//! Modal application state, independent of terminal setup and the worker.
//!
//! Visible indices and wrapped reader lines are cached. Input mutates explicit
//! state; drawing performs no SQL, HTTP or parsing. Refresh preserves the
//! selected article identity whenever it still belongs to the current view.

mod annotations;
mod catalog_worker;
mod commands;
mod completion;
mod configuration;
mod deletion;
mod document_view;
mod documents;
mod draft_commands;
mod draft_keys;
mod draft_save;
mod editor;
mod events;
mod exchange;
mod extraction;
mod help;
mod images;
mod input;
mod layouts;
mod navigation;
mod navigation_worker;
mod note_actions;
mod note_catalog;
mod note_io;
mod note_marks;
mod note_refresh;
mod notebook;
mod organization;
mod pane_motion;
mod panel_docking;
mod panels;
mod passages;
mod personal_tasks;
mod picker;
mod pointer;
mod read_state;
mod reader_tools;
mod reading;
mod reading_data;
mod recommendations;
mod recovery;
mod session_writer;
mod settings;
pub(crate) use settings::validate_binding;
mod study;
mod vim;
mod workspace;

pub use annotations::{AnnotationEntry, AnnotationPosition};
pub use completion::{CommandSpec, command_suggestions};
pub use document_view::DocumentView;
pub use draft_keys::EditMode;
pub use events::InputBatch;
pub use help::HELP;
pub use images::ImageInspector;
pub use images::ReaderImages;
pub use notebook::{NoteAction, NoteContext, Notebook, ReviewTarget};
pub use pane_motion::FloatingPane;
use personal_tasks::Edit as NoteEdit;
pub use picker::{Choice, ChoiceAction, Picker, PickerKind, WorkspaceCommand};
pub use pointer::PanelDrag;
pub use reader_tools::{ReaderLocation, ReaderTools};
pub use settings::{SettingsDraft, key_name};
pub use study::StudyAction;
pub use workspace::SmartForm;

use anyhow::Result;
use crossterm::event::KeyEvent;

use crate::{
    model::{Article, Comment, Library},
    store::Store,
};

pub use crate::model::Pane as Focus;

pub use crate::model::View;

/// A selectable navigation row.
pub struct NavItem {
    /// Destination filter.
    pub view: View,
    /// Display name.
    pub title: String,
    /// Section label; repeated labels are grouped by the renderer.
    pub group: &'static str,
    /// Number of articles across the complete destination.
    pub count: usize,
}

/// An editing operation captures its target before background refresh occurs.
#[derive(Clone, Debug)]
pub enum InputKind {
    /// Live search within the independent annotation notebook.
    NoteSearch,
    /// Independent tags on one captured annotation.
    NoteTags {
        /// Owning article.
        article: i64,
        /// Captured annotation.
        note: i64,
    },
    /// Save the active note query as a named dynamic collection.
    NoteCollection,
    /// Live metadata search.
    Search,
    /// Search only the current publisher text without changing library scope.
    ReaderSearch,
    /// Ex-style command line.
    Command,
    /// Create an empty list.
    NewList,
    /// Save an article in a named list, creating that list if necessary.
    AddToList(i64),
    /// Create or edit a private comment on a captured article.
    Comment {
        /// Article being annotated.
        article: i64,
        /// Existing comment identity, or a new comment.
        comment: Option<i64>,
    },
    /// Highlighted passage captured from the reader.
    Highlight {
        /// Captured source identity.
        article: i64,
        /// Immutable quote, source identity, revision and byte anchor.
        passage: crate::model::Passage,
    },
    /// Website/feed discovery.
    Discover,
    /// OPML import path.
    Import,
    /// Current article Markdown export path.
    ExportArticle(i64),
    /// Reading notebook Markdown export path.
    ExportNotebook,
    /// OPML export path.
    ExportOpml,
    /// User-authored question and answer for a captured annotation.
    StudyCard {
        /// Annotation's owning article.
        article: i64,
        /// Captured annotation identity.
        comment: i64,
        /// Saved card identity after the first write, or a new card.
        card: Option<i64>,
    },
}

/// Modal draft with native cursor, selection and bounded edit history.
#[derive(Clone, Debug)]
pub struct Input {
    /// Operation to submit.
    pub kind: InputKind,
    /// Editable UTF-8 text.
    pub text: String,
    /// Search to restore when a live search is canceled.
    pub previous_search: String,
    /// Cursor, selection and bounded native edit history.
    pub editor: tui_textarea::TextArea<'static>,
    /// Selected command suggestion; reset whenever the draft changes.
    pub completion: usize,
    /// Editing layer within a note or study-card buffer.
    pub edit_mode: EditMode,
    /// Last successfully written text, used to protect modified buffers.
    pub saved_text: String,
    pub(crate) initial_kind: Option<crate::model::NoteKind>,
}

/// Current keyboard mode.
#[derive(Clone, Debug, Default)]
pub enum Mode {
    /// Vim-inspired navigation and actions.
    #[default]
    Normal,
    /// Help overlay.
    Help,
    /// Search, command or annotation editor.
    Input(Box<Input>),
    /// Searchable command/article/list/link/source selector.
    Picker(Box<Picker>),
    /// Contextual Space-key menu.
    Leader,
    /// Typed smart-list form.
    Smart(Box<SmartForm>),
    /// Scrollable read-only document with an optional captured confirmation.
    Document(Box<DocumentView>),
    /// Reversible live preview of visual preferences.
    Settings(Box<SettingsDraft>),
    /// Source-based image inspection with bounded zoom and pan.
    Image(Box<ImageInspector>),
}

/// Side effects that the terminal runner owns.
#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    /// Keep processing input.
    None,
    /// Exit after restoring the terminal.
    Quit,
    /// Ask the worker to collect subscriptions.
    Refresh,
    /// Reconcile declarative definitions away from keyboard and drawing paths.
    ReloadConfiguration,
    /// Open a validated article URL in the system browser.
    Open(String),
    /// Fetch discovery choices away from the input loop.
    Discover(String),
    /// Suspend the terminal and edit the current captured draft.
    ExternalEditor,
    /// Request a write to the terminal's system clipboard.
    Copy(String),
    /// Download/extract a captured article on the dedicated article worker.
    FetchArticle(crate::model::ArticleTarget),
    /// Preview a source extraction rule without saving it.
    PreviewRule(crate::model::RuleRequest),
}

/// State owned by the UI thread. Drawing reads this without performing I/O.
pub struct App {
    configuration_document: Option<crate::config::Update>,
    /// Independent annotation workspace, suspended while a draft is open.
    pub notebook: Option<Box<Notebook>>,
    notebook_return: Option<Box<Notebook>>,
    pub(crate) store: Store,
    pub(crate) layouts: crate::model::LayoutHistory,
    /// Bounded, body-free library snapshot.
    pub library: Library,
    /// Navigation destinations in display order.
    pub navigation: Vec<NavItem>,
    /// Selected navigation row.
    pub nav_selected: usize,
    /// Current article filter.
    pub view: View,
    /// Current keyboard focus.
    pub focus: Focus,
    /// Current modal editor state.
    pub mode: Mode,
    /// Search terms, matched against headline, feed and author.
    pub search: String,
    /// Matching indices in `library.articles`.
    pub visible: Vec<usize>,
    /// Selected position in `visible`.
    pub selected: usize,
    /// Cached body of the selected article.
    pub body: String,
    /// Cached feed-content availability; never inferred during drawing.
    pub body_kind: crate::content::BodyKind,
    /// Source currently displayed, which can fall back from Full to Feed.
    pub content_source: crate::model::ContentSource,
    /// Current Full body revision, or zero for Feed.
    pub content_revision: i64,
    /// Selected article's independent cache and last fetch outcome.
    pub document: Option<crate::model::CachedDocument>,
    /// Parser-recorded Feed/Summary/Missing origin of the selected entry.
    pub feed_origin: crate::model::FeedOrigin,
    /// Cached extraction quality warning, outside quotation/source mappings.
    pub article_warning: String,
    pub(crate) pending_rule: Option<crate::model::RulePreview>,
    /// Automatic fetching policy of the selected subscription.
    pub fetch_policy: crate::model::FetchPolicy,
    /// Captured target of the one outstanding article fetch.
    pub article_fetching: Option<crate::model::ArticleTarget>,
    pub(crate) pending_document: Option<crate::model::ArticleTarget>,
    pub(crate) last_article_attempt: Option<(crate::model::ArticleTarget, i64)>,
    pub(crate) feed_body_kind: crate::content::BodyKind,
    pub(crate) feed_links: Vec<(String, String)>,
    /// Cached reading-time estimate at 220 words per minute.
    pub reading_minutes: usize,
    /// Comments belonging to the selected article.
    pub comments: Vec<Comment>,
    /// Cached article-local annotation rows in resolved source order.
    pub annotations: Vec<AnnotationEntry>,
    note_marks: note_marks::NoteMarks,
    /// Selected comment for edit/delete operations.
    pub comment_selected: usize,
    /// Cached wrapped lines, refreshed on selection or width changes.
    pub reader_lines: Vec<String>,
    /// First visible reader line.
    pub scroll: usize,
    /// Current reader width in terminal columns.
    pub reader_width: usize,
    /// Visible body rows, used to clamp scrolling to a full final page.
    pub reader_height: usize,
    /// User-facing status, including recoverable failures.
    pub status: String,
    /// Whether a collection pass is outstanding.
    pub refreshing: bool,
    /// Whether the current database is an isolated demonstration.
    pub demo: bool,
    pub(crate) pending_g: bool,
    pub(crate) fullscreen_focus: Option<Focus>,
    pub(crate) retained_article: Option<i64>,
    pub(crate) pending_anchor: Option<usize>,
    /// Persistent layout and session preferences.
    pub workspace: crate::model::Workspace,
    /// Current page's absolute offset.
    pub page_offset: usize,
    /// Total matching rows across the entire library.
    pub page_total: usize,
    /// Requested metadata page size, clamped to Store's 1–500 row contract.
    pub page_size: usize,
    /// Readable-text byte anchor in visual selection mode.
    pub selection: Option<usize>,
    /// Active reader selection cursor.
    pub reader_cursor: usize,
    /// Grapheme-aligned byte within the active display row.
    pub reader_column: usize,
    /// Preferred terminal column while traversing short rows.
    pub(crate) reader_goal: Option<usize>,
    /// Whether visual selection includes complete display rows.
    pub linewise: bool,
    /// Readable text before soft wrapping, used by precise selections.
    pub reader_text: String,
    /// Cached article structure, local search and source-bound jump history.
    pub reader_tools: ReaderTools,
    /// Separate image loading state; text layout never waits for it.
    pub images: ReaderImages,
    renderer: crate::reader::LiveRenderer,
    navigation_worker: Option<navigation_worker::NavigationWorker>,
    catalog_worker: Option<catalog_worker::CatalogWorker>,
    session_writer: Option<session_writer::SessionWriter>,
    personal_writes: Option<personal_tasks::PersonalWrites>,
    note_catalog: Option<note_catalog::NoteCatalog>,
    recommendations: Option<recommendations::Recommendations>,
    pending_note_write: Option<personal_tasks::PendingWrite>,
    pending_note_deletes: std::collections::BTreeMap<u64, i64>,
    recovery: recovery::Recovery,
    note_inspector: Option<notebook::inspection::Inspector>,
    suspended_note: Option<Box<Input>>,
    reading_article: Option<i64>,
    displayed_article: Option<Article>,
    catalog_loading: bool,
    library_revision: u64,
    read_reconcile_required: bool,
    projecting_navigation: bool,
    pending_reads: std::collections::BTreeMap<u64, (Vec<i64>, bool)>,
    immediate_layout: bool,
    pending_reflow: bool,
    progress_memory: std::cell::RefCell<session_writer::ProgressMemory>,
    /// Selected metadata/content is being loaded without blocking input.
    pub navigation_loading: bool,
    vim: vim::Vim,
    /// Readable-text byte ranges corresponding to display rows.
    pub reader_ranges: Vec<std::ops::Range<usize>>,
    /// Parser-owned correspondence for article quotations.
    pub reader_mappings: Vec<crate::reader::SourceMapping>,
    /// End of article text before private notes are appended.
    pub(crate) article_text_end: usize,
    /// Original source ranges for selectable rendered lines.
    pub line_ranges: Vec<std::ops::Range<usize>>,
    pub(crate) reader_columns: Vec<Option<crate::reader::viewport::RowIndex>>,
    /// Cached rich lines; drawing does not parse or highlight.
    pub reader_styled: Vec<ratatui::text::Line<'static>>,
    /// HTTP(S) references extracted from the current content.
    pub links: Vec<(String, String)>,
    /// Horizontal code/table scroll in terminal columns.
    pub horizontal: usize,
    /// Display rows that preserve columns, including code blocks and tables.
    pub code_lines: Vec<bool>,
    /// Display row ranges of tables, which keep the ordinary panel background.
    pub table_rows: Vec<std::ops::Range<usize>>,
    /// Scrollable help position.
    pub help_scroll: usize,
    /// Captured article identities for a reversible bulk operation.
    pub marked: std::collections::BTreeSet<i64>,
    screen: ratatui::layout::Rect,
    pointer: pointer::Pointer,
    panel_prefix: bool,
    pane_flight: Option<pane_motion::PaneFlight>,
    pane_neighbors: Option<pane_motion::Neighbors>,
}

impl App {
    /// Load a local library and its initial inbox view.
    ///
    /// # Errors
    /// Returns storage or content-loading errors.
    #[expect(
        clippy::too_many_lines,
        reason = "Keep complete initial state visible together before loading its caches"
    )]
    pub fn new(store: Store, demo: bool) -> Result<Self> {
        let library = store.catalog()?;
        let workspace = store.workspace()?;
        let page_offset = workspace.page_offset;
        let retained_article = if workspace.view == View::Inbox {
            workspace.article
        } else {
            None
        };
        let layouts = store.layouts(&workspace)?;
        let mut app = Self {
            configuration_document: None,
            notebook: None,
            notebook_return: None,
            store,
            library,
            navigation: Vec::new(),
            nav_selected: 0,
            view: workspace.view.clone(),
            focus: if workspace.fullscreen {
                Focus::Reader
            } else {
                Focus::Articles
            },
            mode: Mode::Normal,
            search: String::new(),
            visible: Vec::new(),
            selected: 0,
            body: String::new(),
            body_kind: crate::content::BodyKind::Missing,
            content_source: crate::model::ContentSource::Feed,
            content_revision: 0,
            document: None,
            feed_origin: crate::model::FeedOrigin::Feed,
            article_warning: String::new(),
            pending_rule: None,
            fetch_policy: crate::model::FetchPolicy::Missing,
            article_fetching: None,
            pending_document: None,
            last_article_attempt: None,
            feed_body_kind: crate::content::BodyKind::Missing,
            feed_links: Vec::new(),
            reading_minutes: 0,
            comments: Vec::new(),
            annotations: Vec::new(),
            note_marks: note_marks::NoteMarks::default(),
            comment_selected: 0,
            reader_lines: Vec::new(),
            scroll: 0,
            reader_width: 72,
            reader_height: 20,
            status:
                "Welcome to Lume · ? for shortcuts · :feed URL to subscribe"
                    .into(),
            refreshing: false,
            demo,
            pending_g: false,
            fullscreen_focus: None,
            retained_article,
            layouts,
            pending_anchor: None,
            workspace,
            page_offset,
            page_total: 0,
            page_size: 200,
            selection: None,
            reader_cursor: 0,
            reader_column: 0,
            reader_goal: None,
            linewise: false,
            reader_text: String::new(),
            reader_tools: ReaderTools::default(),
            images: ReaderImages::default(),
            renderer: crate::reader::LiveRenderer::start()?,
            navigation_worker: None,
            catalog_worker: None,
            session_writer: None,
            personal_writes: None,
            note_catalog: None,
            recommendations: None,
            pending_note_write: None,
            pending_note_deletes: std::collections::BTreeMap::new(),
            recovery: recovery::Recovery::default(),
            note_inspector: None,
            suspended_note: None,
            reading_article: None,
            displayed_article: None,
            catalog_loading: false,
            library_revision: 0,
            read_reconcile_required: false,
            projecting_navigation: false,
            pending_reads: std::collections::BTreeMap::new(),
            immediate_layout: false,
            pending_reflow: false,
            progress_memory: std::cell::RefCell::default(),
            navigation_loading: false,
            vim: vim::Vim::default(),
            reader_ranges: Vec::new(),
            reader_mappings: Vec::new(),
            article_text_end: 0,
            line_ranges: Vec::new(),
            reader_columns: Vec::new(),
            reader_styled: Vec::new(),
            links: Vec::new(),
            horizontal: 0,
            code_lines: Vec::new(),
            table_rows: Vec::new(),
            help_scroll: 0,
            marked: std::collections::BTreeSet::new(),
            screen: ratatui::layout::Rect::default(),
            pointer: pointer::Pointer::default(),
            panel_prefix: false,
            pane_flight: None,
            pane_neighbors: None,
        };
        app.rebuild_navigation()?;
        app.filter()?;
        if let Some(article) = app.workspace.article {
            app.restore_article(article)?;
        }
        app.ensure_visible_focus();
        let drafts = app.store.note_draft_count()?;
        if drafts > 0 {
            app.status = format!(
                "{drafts} recoverable drafts · :drafts resumes editing"
            );
        }
        Ok(app)
    }

    /// Return the selected article, if the current view has any matches.
    pub fn current(&self) -> Option<&Article> {
        self.visible
            .get(self.selected)
            .and_then(|index| self.library.articles.get(*index))
    }

    /// Handle a terminal key without performing network I/O.
    ///
    /// # Errors
    /// Returns recoverable storage or command-validation failures.
    pub fn key(&mut self, key: KeyEvent) -> Result<Action> {
        let action = self.handle_key(key);
        self.observe_draft();
        action
    }

    /// Reload metadata after collection, preserving selection by identity.
    ///
    /// # Errors
    /// Returns SQLite or selected-content errors.
    pub fn reload(&mut self) -> Result<()> {
        self.library_revision = self.library_revision.wrapping_add(1);
        if self.navigation_worker.is_some() {
            self.save_reading_position()?;
            self.catalog_loading = true;
            self.request_catalog(catalog_worker::Scope::Library);
            self.invalidate_notebook();
            return Ok(());
        }
        self.navigation_loading = false;
        self.invalidate_notebook();
        self.reload_now()
    }

    fn reload_now(&mut self) -> Result<()> {
        self.save_reading_position()?;
        let selected_id = self.current().map(|article| article.id);
        let reader = self.capture_reader();
        self.library = self.store.catalog()?;
        self.rebuild_navigation()?;
        self.filter_now()?;
        if let Some(id) = selected_id {
            self.restore_page_selection(id)?;
        }
        self.restore_reader(reader);
        Ok(())
    }

    /// Reflow only when the reader width changes.
    pub fn resize_reader(&mut self, width: usize, height: usize) {
        let width = width.max(1);
        if self.reader_width == width
            && self.reader_height == height.max(1)
            && self.pending_anchor.is_none()
        {
            return;
        }
        let cursor = self.cursor_offset();
        let pending = self.pending_anchor.take();
        let anchor = pending.unwrap_or_else(|| {
            self.line_ranges
                .get(self.scroll)
                .map_or(0, |range| range.start)
        });
        self.reader_height = height.max(1);
        let reflow = self.reader_width != width;
        if reflow {
            self.reader_width = width;
            if !self.navigation_loading && !self.projecting_navigation {
                self.wrap_reader();
            }
        }
        if self.renderer.preparing(&self.body, self.reader_width) {
            self.pending_reflow |= pending.is_none() && reflow;
            self.pending_anchor = Some(anchor);
            self.clamp_scroll();
            return;
        }
        let preserve_cursor =
            self.pending_reflow || reflow && pending.is_none();
        self.pending_reflow = false;
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
