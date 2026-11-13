//! Modal application state, independent of terminal setup and the worker.
//!
//! Visible indices and wrapped reader lines are cached. Input mutates explicit
//! state; drawing performs no SQL, HTTP or parsing. Refresh preserves the
//! selected article identity whenever it still belongs to the current view.

mod annotations;
mod article_edits;
mod article_requests;
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
mod interaction_state;
mod layout_state;
pub use layout_state::LayoutView;
mod layouts;
mod maintenance;
mod navigation;
mod navigation_sections;
mod navigation_state;
pub use navigation_state::NavigationView;
mod navigation_worker;
mod network_state;
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
mod personal_state;
mod personal_tasks;
mod picker;
mod picker_reads;
mod pointer;
mod presentation;
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
mod session_state;
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
pub use help::{HELP, HelpView};
pub use images::ImageInspector;
pub use images::ReaderImages;
pub use notebook::{NoteAction, NoteContext, NotebookView, ReviewTarget};
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
    model::{Article, Comment},
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
    layout: layout_state::LayoutState,
    nav: navigation_state::NavigationState,
    configuration: configuration::ConfigurationState,
    notebooks: notebook::NotebookState,
    store: Store,
    picker_reads: Option<picker_reads::PickerReads>,
    performance: performance::Performance,
    web: web_reading::WebReading,
    network: network_state::NetworkState,
    maintenance: maintenance::MaintenanceState,
    interaction: interaction_state::InteractionState,
    article_requests: article_requests::ArticleRequests,
    /// Separate image loading state; text layout never waits for it.
    images: ReaderImages,
    headlines: std::cell::RefCell<headlines::Headlines>,
    navigation_worker: Option<navigation_worker::NavigationWorker>,
    catalog_worker: Option<catalog_worker::CatalogWorker>,
    session: session_state::SessionState,
    personal: personal_state::PersonalState,
    note_catalog: Option<note_catalog::NoteCatalog>,
    recommendations: Option<recommendations::Recommendations>,
    pointer: pointer::Pointer,
    pane_motion: pane_motion::PaneMotion,
}

impl App {
    /// Borrow the active annotation workspace without exposing its read workers.
    pub fn notebook(&self) -> Option<NotebookView<'_>> {
        self.notebooks.view()
    }

    /// Borrow cached library and navigation presentation without I/O.
    pub fn navigation(&self) -> NavigationView<'_> {
        self.nav.presentation()
    }

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
            nav: navigation_state::NavigationState::new(
                library,
                workspace.view.clone(),
                page_offset,
                retained_article,
                !eager,
            ),
            configuration: configuration::ConfigurationState::default(),
            notebooks: notebook::NotebookState::default(),
            store,
            picker_reads: None,
            performance: performance::Performance::default(),
            web: web_reading::WebReading::default(),
            network: network_state::NetworkState::new(demo),
            maintenance: maintenance::MaintenanceState::default(),
            interaction: interaction_state::InteractionState::default(),
            article_requests: article_requests::ArticleRequests::default(),
            layout: layout_state::LayoutState::new(workspace, layouts),
            images: ReaderImages::default(),
            headlines: std::cell::RefCell::default(),
            navigation_worker: None,
            catalog_worker: None,
            session: session_state::SessionState::default(),
            personal: personal_state::PersonalState::default(),
            note_catalog: None,
            recommendations: None,
            pointer: pointer::Pointer::default(),
            pane_motion: pane_motion::PaneMotion::default(),
        };
        if eager {
            app.rebuild_navigation()?;
            app.filter()?;
            if let Some(article) = app.layout.workspace().article {
                app.restore_article(article)?;
            }
        } else {
            app.install_navigation(navigation::pending_navigation(
                app.navigation().library,
                app.layout.workspace().last_visit,
            ));
        }
        app.apply_image_preferences();
        app.ensure_visible_focus();
        let drafts = app.store.note_draft_count()?;
        if drafts > 0 {
            app.interaction.report(format!(
                "{drafts} recoverable drafts · :drafts resumes editing"
            ));
        }
        Ok(app)
    }

    /// Return the selected article, if the current view has any matches.
    pub fn current(&self) -> Option<&Article> {
        self.nav.current()
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
        self.nav.invalidate_library();
        if self.navigation_worker.is_some() {
            self.save_reading_position()?;
            self.nav.begin_catalog();
            self.request_catalog(catalog_worker::Scope::Library);
            self.invalidate_notebook();
            return Ok(());
        }
        self.nav.cancel_loading();
        self.invalidate_notebook();
        self.reload_now()
    }

    fn reload_now(&mut self) -> Result<()> {
        self.save_reading_position()?;
        let selected_id = self.current().map(|article| article.id);
        let reader = self.capture_reader();
        self.nav.replace_catalog(self.store.catalog()?);
        self.rebuild_navigation()?;
        self.filter_now()?;
        if let Some(id) = selected_id {
            self.restore_page_selection(id)?;
        }
        self.restore_reader(reader);
        Ok(())
    }
}
