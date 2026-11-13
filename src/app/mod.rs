//! Modal application state, independent of terminal setup and the worker.
//!
//! Visible indices and wrapped reader lines are cached. Input mutates explicit
//! state; drawing performs no SQL, HTTP or parsing. Refresh preserves the
//! selected article identity whenever it still belongs to the current view.

mod annotations;
mod article_edits;
mod auto_read;
mod catalog_worker;
mod commands;
mod completion;
mod configuration;
mod deletion;
mod destination_deletion;
mod document_view;
mod documents;
mod draft_commands;
mod draft_keys;
mod draft_save;
mod editor;
mod events;
mod exchange;
mod extraction;
mod formulas;
mod headlines;
mod help;
pub(crate) use headlines::Labels as HeadlineLabels;
mod images;
mod input;
mod layouts;
mod maintenance;
mod navigation;
mod navigation_sections;
mod navigation_worker;
mod note_actions;
mod note_catalog;
mod note_io;
mod note_marks;
mod note_refresh;
mod notebook;
mod organization;
mod pane_motion;
mod pane_search;
mod panel_docking;
mod panels;
mod passages;
mod performance;
mod servers;
pub use performance::Operation;
pub use servers::ServerToken;
mod personal_tasks;
mod picker;
mod picker_reads;
mod pointer;
mod reader_links;
mod reader_state;
mod reader_state_api;
mod reader_tools;
pub use reader_state::ReaderView;
mod reading;
pub use reading::ReaderIdentity;
mod reading_data;
mod recommendations;
mod recovery;
mod session_writer;
mod settings;
mod study;
mod vim;
mod web_reading;
mod workspace;
pub use web_reading::{WebDocument, WebPage};

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
pub use reader_state::ReaderTools;
pub use reader_tools::ReaderLocation;
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
#[derive(Clone)]
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
    /// In-memory name search within sidebar destinations.
    LibrarySearch,
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
    /// A temporary linked reading with no article mutation identity.
    Web,
    /// Vim-inspired navigation and actions.
    #[default]
    Normal,
    /// Help overlay.
    Help,
    /// Search, command or annotation editor.
    Input(Box<Input>),
    /// Masked API token entry, excluded from ordinary drafts and command text.
    ServerToken(ServerToken),
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
    reader: reader_state::ReaderState,
    configuration: configuration::ConfigurationState,
    /// Independent annotation workspace, suspended while a draft is open.
    pub notebook: Option<Box<Notebook>>,
    notebook_return: Option<Box<Notebook>>,
    pub(crate) store: Store,
    pub(crate) layouts: crate::model::LayoutHistory,
    /// Bounded, body-free library snapshot.
    pub library: Library,
    /// Navigation destinations in display order.
    pub navigation: Vec<NavItem>,
    navigation_sections: navigation_sections::Sections,
    library_search: pane_search::LibrarySearch,
    picker_reads: Option<picker_reads::PickerReads>,
    performance: performance::Performance,
    web: web_reading::WebReading,
    network_enabled: bool,
    maintenance: Option<maintenance::Maintenance>,
    storage_diagnostics: Option<maintenance::Diagnostics>,
    /// Selected navigation row.
    pub nav_selected: usize,
    /// Transient Library viewport: filtered entry position and leading rows.
    pub(crate) navigation_first: usize,
    pub(crate) navigation_skip: usize,
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
    /// First visible article-page position; drawing and pointer hits share it.
    pub(crate) articles_first: usize,
    pub(crate) pending_rule: Option<crate::model::RulePreview>,
    /// Captured target of the one outstanding article fetch.
    pub article_fetching: Option<crate::model::ArticleTarget>,
    pub(crate) pending_document: Option<crate::model::ArticleTarget>,
    pub(crate) last_article_attempt: Option<(crate::model::ArticleTarget, i64)>,
    /// User-facing status, including recoverable failures.
    pub status: String,
    /// Whether a collection pass is outstanding.
    pub refreshing: bool,
    /// Whether the current database is an isolated demonstration.
    pub demo: bool,
    pub(crate) pending_g: bool,
    pending_destination_delete: Option<View>,
    pending_article_delete: Option<i64>,
    pub(crate) fullscreen_focus: Option<Focus>,
    pub(crate) retained_article: Option<i64>,
    /// Persistent layout and session preferences.
    pub workspace: crate::model::Workspace,
    /// Current page's absolute offset.
    pub page_offset: usize,
    /// Total matching rows across the entire library.
    pub page_total: usize,
    /// Requested metadata page size, clamped to Store's 1–500 row contract.
    pub page_size: usize,
    /// Separate image loading state; text layout never waits for it.
    pub images: ReaderImages,
    headlines: std::cell::RefCell<headlines::Headlines>,
    navigation_worker: Option<navigation_worker::NavigationWorker>,
    catalog_worker: Option<catalog_worker::CatalogWorker>,
    session_writer: Option<session_writer::SessionWriter>,
    personal_writes: Option<personal_tasks::PersonalWrites>,
    note_catalog: Option<note_catalog::NoteCatalog>,
    recommendations: Option<recommendations::Recommendations>,
    pending_note_write: Option<personal_tasks::PendingWrite>,
    /// A consumed failure survives an incomplete personal-write flush until the
    /// next completed flush or ordinary note poll reports it exactly once.
    note_flush_failure: Option<anyhow::Error>,
    pending_note_deletes: std::collections::BTreeMap<u64, i64>,
    recovery: recovery::Recovery,
    note_inspector: Option<notebook::inspection::Inspector>,
    suspended_note: Option<Box<Input>>,
    catalog_loading: bool,
    library_revision: u64,
    projecting_navigation: bool,
    article_edits: article_edits::ArticleEdits,
    immediate_layout: bool,
    progress_memory: std::cell::RefCell<session_writer::ProgressMemory>,
    /// Selected metadata/content is being loaded without blocking input.
    pub navigation_loading: bool,
    vim: vim::Vim,
    session_problem: Option<String>,
    /// Scrollable help position.
    pub help_scroll: usize,
    pub(crate) help_search: help::HelpSearch,
    /// Captured article identities for a reversible bulk operation.
    pub marked: std::collections::BTreeSet<i64>,
    screen: ratatui::layout::Rect,
    pointer: pointer::Pointer,
    panel_prefix: bool,
    pane_flight: Option<pane_motion::PaneFlight>,
    pane_neighbors: Option<pane_motion::Neighbors>,
}

impl App {
    /// Borrow cached reader presentation without parsing or I/O.
    pub fn reader(&self) -> ReaderView<'_> {
        let mut view = self.reader.view();
        if !self.reader_stale() {
            view.displayed_article = self.current();
        }
        view
    }

    /// Load a local library and its initial inbox view.
    ///
    /// # Errors
    /// Returns storage or content-loading errors.
    pub fn new(store: Store, demo: bool) -> Result<Self> {
        Self::initialize(store, demo, true)
    }

    /// Restore preferences without synchronously counting or rendering articles.
    /// Call `load_workspace` after enabling background navigation.
    ///
    /// # Errors
    /// Returns preference, catalog metadata or layout decoding failures.
    pub fn open_interactive(store: Store, demo: bool) -> Result<Self> {
        Self::initialize(store, demo, false)
    }

    #[expect(
        clippy::too_many_lines,
        reason = "Keep complete initial state visible together before loading its caches"
    )]
    fn initialize(store: Store, demo: bool, eager: bool) -> Result<Self> {
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
            reader: reader_state::ReaderState::new()?,
            configuration: configuration::ConfigurationState::default(),
            notebook: None,
            notebook_return: None,
            store,
            library,
            navigation: Vec::new(),
            navigation_sections: navigation_sections::Sections::default(),
            library_search: pane_search::LibrarySearch::default(),
            picker_reads: None,
            performance: performance::Performance::default(),
            web: web_reading::WebReading::default(),
            network_enabled: false,
            maintenance: None,
            storage_diagnostics: None,
            nav_selected: 0,
            navigation_first: 0,
            navigation_skip: 0,
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
            articles_first: 0,
            pending_rule: None,
            article_fetching: None,
            pending_document: None,
            last_article_attempt: None,
            status:
                "Welcome to Lume · ? for shortcuts · :feed URL to subscribe"
                    .into(),
            refreshing: false,
            demo,
            pending_g: false,
            pending_destination_delete: None,
            pending_article_delete: None,
            fullscreen_focus: None,
            retained_article,
            layouts,
            workspace,
            page_offset,
            page_total: 0,
            page_size: 200,
            images: ReaderImages::default(),
            headlines: std::cell::RefCell::default(),
            navigation_worker: None,
            catalog_worker: None,
            session_writer: None,
            personal_writes: None,
            note_catalog: None,
            recommendations: None,
            pending_note_write: None,
            note_flush_failure: None,
            pending_note_deletes: std::collections::BTreeMap::new(),
            recovery: recovery::Recovery::default(),
            note_inspector: None,
            suspended_note: None,
            catalog_loading: false,
            library_revision: 0,
            projecting_navigation: false,
            article_edits: article_edits::ArticleEdits::default(),
            immediate_layout: false,
            progress_memory: std::cell::RefCell::default(),
            navigation_loading: !eager,
            vim: vim::Vim::default(),
            session_problem: None,
            help_scroll: 0,
            help_search: help::HelpSearch::default(),
            marked: std::collections::BTreeSet::new(),
            screen: ratatui::layout::Rect::default(),
            pointer: pointer::Pointer::default(),
            panel_prefix: false,
            pane_flight: None,
            pane_neighbors: None,
        };
        if eager {
            app.rebuild_navigation()?;
            app.filter()?;
            if let Some(article) = app.workspace.article {
                app.restore_article(article)?;
            }
        } else {
            app.install_navigation(navigation::pending_navigation(
                &app.library,
                app.workspace.last_visit,
            ));
        }
        app.apply_image_preferences();
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
        let motion = self.pending_formula_motion();
        let action = self.handle_key(key);
        self.retarget_formula_motion(motion);
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
}
