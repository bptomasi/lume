//! A paged annotation workspace separates note search from source context.
//!
//! Independent latest-read workers reject obsolete list/context results.
//! Mutations never use these workers. Standard pane identities retain docking,
//! keyboard resizing and collapse while displaying notebook responsibilities.

mod actions;
mod commands;
mod editor_actions;
pub(super) mod inspection;
mod interaction;
mod keys;
mod readers;
mod references;
mod review;
mod templates;

pub use readers::NoteContext;
pub use references::NoteAction;
pub use review::ReviewTarget;

use anyhow::Result;

use crate::model::{NoteCollection, NotePage, NoteSummary, Workspace};

use super::{App, Focus};

/// Notebook query, layout and cached previews; drawing performs no I/O.
pub struct Notebook {
    /// Note query, independent of article search.
    pub query: String,
    /// Bounded note preview window.
    pub page: NotePage,
    /// Absolute offset of the page.
    pub offset: usize,
    /// Selected note within the page.
    pub selected: usize,
    /// User-authored dynamic collections, after four built-in destinations.
    pub collections: Vec<NoteCollection>,
    /// Selected collection row.
    pub collection: usize,
    /// Selected note and its safely located source excerpt.
    pub context: Option<NoteContext>,
    /// Selected incoming/outgoing link in the optional backlinks pane.
    pub link_selected: usize,
    /// First context display row.
    pub scroll: usize,
    /// Most recent successful explicit searches.
    pub recent: Vec<String>,
    recent_generation: u64,
    /// Captured note identities selected for a synthesis.
    pub marked: std::collections::BTreeSet<i64>,
    /// A list query is pending; cached results remain available.
    pub loading: bool,
    /// A source context read is pending.
    pub context_loading: bool,
    /// Validation/read failure outside user text.
    pub error: String,
    pub(super) article: Option<i64>,
    pub(super) width: usize,
    pub(super) pending_delete: Option<i64>,
    before: Workspace,
    before_layouts: crate::model::LayoutHistory,
    focus_before: Focus,
    readers: readers::Readers,
    history: Vec<(String, usize, usize)>,
    loaded_query: String,
    loaded_offset: usize,
}

impl Notebook {
    fn start(app: &App, query: &str, article: Option<i64>) -> Result<Self> {
        let path = app.store.database_path()?;
        Ok(Self {
            query: query.into(),
            page: NotePage::default(),
            offset: 0,
            selected: 0,
            collections: Vec::new(),
            collection: 0,
            context: None,
            scroll: 0,
            link_selected: 0,
            recent: Vec::new(),
            recent_generation: 0,
            marked: std::collections::BTreeSet::new(),
            loading: false,
            context_loading: false,
            error: String::new(),
            article,
            width: 72,
            pending_delete: None,
            before: app.workspace.clone(),
            before_layouts: app.layouts.clone(),
            focus_before: app.focus,
            readers: readers::Readers::start(&path)?,
            history: Vec::new(),
            loaded_query: String::new(),
            loaded_offset: 0,
        })
    }

    /// Selected note preview, including while another read is pending.
    pub fn current(&self) -> Option<&NoteSummary> {
        if self.loaded_query != self.query || self.loaded_offset != self.offset
        {
            return None;
        }
        self.page.notes.get(self.selected)
    }

    /// Context is available only for the exact currently selected note.
    pub fn current_context(&self) -> Option<&NoteContext> {
        let id = self.current()?.id;
        self.context
            .as_ref()
            .filter(|item| item.note.comment.id == id)
    }

    fn refresh(&mut self) -> Result<()> {
        crate::search::notes::validate(&self.query)?;
        self.readers.list.submit(readers::ListRequest {
            text: self.query.clone(),
            article: self.article,
            offset: self.offset,
            recent_generation: self.recent_generation,
        });
        self.loading = true;
        self.error.clear();
        Ok(())
    }

    pub(super) fn install_recent(&mut self, recent: Vec<String>) {
        // A late list snapshot must not overwrite an acknowledged history edit.
        self.recent_generation = self.recent_generation.wrapping_add(1);
        self.recent = recent;
    }

    fn install_listing(&mut self, listing: readers::Listing) {
        let preserve = self.query == self.loaded_query
            && self.offset == self.loaded_offset;
        let id = self.current().filter(|_| preserve).map(|note| note.id);
        self.loaded_query.clone_from(&self.query);
        self.loaded_offset = listing.offset;
        self.page = listing.page;
        self.offset = listing.offset;
        self.collections = listing.collections;
        self.collection = self.collection.min(self.collections.len() + 3);
        if self.recent_generation == listing.recent_generation {
            self.recent = listing.recent;
        }
        self.selected = id
            .and_then(|id| {
                self.page.notes.iter().position(|note| note.id == id)
            })
            .unwrap_or(
                self.selected.min(self.page.notes.len().saturating_sub(1)),
            );
        let scroll = self.scroll;
        self.read_context();
        if self.current().is_some_and(|note| Some(note.id) == id) {
            self.scroll = scroll;
        }
    }

    fn read_context(&mut self) {
        let Some(id) = self.current().map(|note| note.id) else {
            self.context = None;
            self.context_loading = false;
            self.readers.context.cancel();
            return;
        };
        self.readers.context.submit((id, self.width));
        self.context_loading = true;
        if self
            .context
            .as_ref()
            .is_none_or(|context| context.note.comment.id != id)
        {
            self.scroll = 0;
            self.link_selected = 0;
        }
    }
}

impl App {
    pub(super) fn invalidate_notebook(&mut self) {
        if let Some(notebook) = &mut self.notebook
            && let Err(error) = notebook.refresh()
        {
            notebook.error = error.to_string();
        }
    }

    pub(super) fn open_notebook(
        &mut self,
        query: &str,
        article: Option<i64>,
    ) -> Result<()> {
        crate::search::notes::validate(query)?;
        if let Some(notebook) = &mut self.notebook {
            notebook.query = query.into();
            notebook.article = article;
            notebook.offset = 0;
            return notebook.refresh();
        }
        self.notebook_return = None;
        let mut notebook = Notebook::start(self, query, article)?;
        notebook.refresh()?;
        self.notebook = Some(Box::new(notebook));
        self.selection = None;
        self.vim.clear_input();
        self.workspace.fullscreen = false;
        self.workspace.zen = false;
        self.workspace.show_library = true;
        self.workspace.show_articles = true;
        self.workspace.show_reader = true;
        self.workspace.show_annotations = false;
        if let Some(layout) = self.workspace.notebook_layout.clone() {
            layout.apply(&mut self.workspace);
        }
        self.layouts = self.notebook_layout_history();
        self.focus = Focus::Articles;
        self.resize_workspace(self.screen);
        self.status =
            "Notebook · / search · e edit · Enter source · ? help".into();
        Ok(())
    }

    pub(super) fn close_notebook(&mut self) {
        if let Some(notebook) = self.notebook.take() {
            self.workspace.notebook_layout =
                Some(crate::model::PanelLayout::capture(&self.workspace));
            crate::model::PanelLayout::capture(&notebook.before)
                .apply(&mut self.workspace);
            self.layouts = notebook.before_layouts;
            self.focus = notebook.focus_before;
        }
    }

    pub(super) fn suspend_notebook_for_source(&mut self) {
        let Some(notebook) = self.notebook.take() else {
            return;
        };
        self.workspace.notebook_layout =
            Some(crate::model::PanelLayout::capture(&self.workspace));
        crate::model::PanelLayout::capture(&notebook.before)
            .apply(&mut self.workspace);
        self.layouts = notebook.before_layouts.clone();
        self.focus = notebook.focus_before;
        self.notebook_return = Some(notebook);
    }

    pub(super) fn return_to_notebook(&mut self) -> Result<bool> {
        let Some(mut notebook) = self.notebook_return.take() else {
            return Ok(false);
        };
        notebook.before = self.workspace.clone();
        notebook.before_layouts = self.layouts.clone();
        notebook.focus_before = self.focus;
        if let Some(layout) = self.workspace.notebook_layout.clone() {
            layout.apply(&mut self.workspace);
        }
        self.layouts = self.notebook_layout_history();
        self.focus = Focus::Articles;
        notebook.refresh()?;
        self.notebook = Some(notebook);
        Ok(true)
    }

    fn notebook_layout_history(&self) -> crate::model::LayoutHistory {
        let mut history = crate::model::LayoutHistory::new(&self.workspace);
        history.profiles.clone_from(&self.layouts.profiles);
        history
    }

    pub(super) fn persist_notebook_layout(&mut self) -> Result<bool> {
        let Some(notebook) = &mut self.notebook else {
            return Ok(false);
        };
        self.layouts
            .observe(crate::model::PanelLayout::capture(&self.workspace));
        notebook
            .before_layouts
            .profiles
            .clone_from(&self.layouts.profiles);
        let mut saved = self.workspace.clone();
        saved.notebook_layout =
            Some(crate::model::PanelLayout::capture(&self.workspace));
        crate::model::PanelLayout::capture(&notebook.before).apply(&mut saved);
        if let Some(writer) = &self.session_writer {
            writer.layout(&saved, &notebook.before_layouts)?;
        } else {
            self.store.save_layouts(&saved, &notebook.before_layouts)?;
        }
        Ok(true)
    }

    fn clamp_notebook_scroll(&mut self) {
        let area = crate::ui::workspace_areas(
            self.screen,
            self.focus,
            &self.workspace,
        )
        .reader;
        if let Some(notebook) = &mut self.notebook
            && let Some(context) = notebook.current_context()
        {
            let links = context.backlinks.len() + context.references.len();
            notebook.link_selected =
                notebook.link_selected.min(links.saturating_sub(1));
            if let Some(area) = area {
                notebook.scroll = notebook.scroll.min(
                    crate::ui::notebook_scroll_limit(notebook, area.height),
                );
            }
        }
    }

    pub(super) fn resize_notebook_context(&mut self, width: usize) {
        if let Some(notebook) = &mut self.notebook {
            let width = width.clamp(10, 512);
            if notebook.width != width {
                notebook.width = width;
                let scroll = notebook.scroll;
                notebook.read_context();
                notebook.scroll = scroll;
            }
        }
        self.clamp_notebook_scroll();
    }

    /// Poll note search/context workers without blocking input.
    ///
    /// # Errors
    /// Returns read failures while retaining existing previews.
    pub fn poll_notebook(&mut self) -> Result<bool> {
        let Some(notebook) = &mut self.notebook else {
            return Ok(false);
        };
        let mut changed = false;
        if let Some(result) = notebook.readers.list.poll() {
            notebook.loading = false;
            changed = true;
            notebook.install_listing(result?);
        }
        if let Some(result) = notebook.readers.context.poll() {
            notebook.context_loading = false;
            notebook.context = Some(result?);
            changed = true;
        }
        if changed {
            self.clamp_notebook_scroll();
        }
        Ok(changed)
    }

    /// Whether note search or its selected source context is still pending.
    pub fn notebook_pending(&self) -> bool {
        self.notebook.as_ref().is_some_and(|notebook| {
            notebook.loading || notebook.context_loading
        })
    }
}

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn acknowledged_search_history_outlives_an_older_listing_snapshot()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = crate::store::Store::open(
            &directory.path().join("library.sqlite3"),
        )?;
        let app = App::new(store, false)?;
        let mut notebook = Notebook::start(&app, "", None)?;
        let previous = listing(0, Vec::new());

        notebook.install_recent(vec!["body:consensus".into()]);
        notebook.install_listing(previous);

        assert_eq!(notebook.recent, ["body:consensus"]);
        let current = listing(1, vec!["body:protocol".into()]);
        notebook.install_listing(current);
        assert_eq!(notebook.recent, ["body:protocol"]);
        Ok(())
    }

    fn listing(generation: u64, recent: Vec<String>) -> readers::Listing {
        readers::Listing {
            page: NotePage::default(),
            offset: 0,
            collections: Vec::new(),
            recent,
            recent_generation: generation,
        }
    }
}
