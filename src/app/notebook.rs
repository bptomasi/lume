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
mod lifecycle;
mod state;
mod view;
pub use view::NotebookView;
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
pub(super) struct Notebook {
    /// Note query, independent of article search.
    query: String,
    /// Bounded note preview window.
    page: NotePage,
    /// Absolute offset of the page.
    offset: usize,
    /// Selected note within the page.
    selected: usize,
    /// User-authored dynamic collections, after four built-in destinations.
    collections: Vec<NoteCollection>,
    /// Selected collection row.
    collection: usize,
    /// Selected note and its safely located source excerpt.
    context: Option<NoteContext>,
    /// Selected incoming/outgoing link in the optional backlinks pane.
    link_selected: usize,
    /// First context display row.
    scroll: usize,
    /// Most recent successful explicit searches.
    recent: Vec<String>,
    recent_generation: u64,
    /// Captured note identities selected for a synthesis.
    marked: std::collections::BTreeSet<i64>,
    /// A list query is pending; cached results remain available.
    loading: bool,
    /// A source context read is pending.
    context_loading: bool,
    /// Validation/read failure outside user text.
    error: String,
    article: Option<i64>,
    width: usize,
    pending_delete: Option<i64>,
    origin: Origin,
    readers: readers::Readers,
    history: Vec<(String, usize, usize)>,
    // Cached rows remain paintable during query/scope replacement, but their
    // action target is available only while all loaded coordinates still match.
    loaded_query: String,
    loaded_offset: usize,
    loaded_article: Option<i64>,
}

impl App {
    pub(super) fn invalidate_notebook(&mut self) {
        self.notebooks.invalidate();
    }

    pub(super) fn open_notebook(
        &mut self,
        query: &str,
        article: Option<i64>,
    ) -> Result<()> {
        let origin =
            Origin::capture(&self.workspace, &self.layouts, self.focus);
        if !self.notebooks.open(
            &self.store.database_path()?,
            query,
            article,
            origin,
        )? {
            return Ok(());
        }
        self.reader.cancel_selection();
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
        if let Some(origin) = self.notebooks.close() {
            self.restore_notebook_origin(origin);
        }
    }

    fn restore_notebook_origin(&mut self, origin: Origin) {
        self.workspace.notebook_layout =
            Some(crate::model::PanelLayout::capture(&self.workspace));
        origin.layout.apply(&mut self.workspace);
        self.layouts = origin.layouts;
        self.focus = origin.focus;
    }

    pub(super) fn suspend_notebook_for_source(&mut self) {
        if let Some(origin) = self.notebooks.suspend_source() {
            self.restore_notebook_origin(origin);
        }
    }

    pub(super) fn return_to_notebook(&mut self) -> Result<bool> {
        let origin =
            Origin::capture(&self.workspace, &self.layouts, self.focus);
        if !self.notebooks.resume_source(origin)? {
            return Ok(false);
        }
        if let Some(layout) = self.workspace.notebook_layout.clone() {
            layout.apply(&mut self.workspace);
        }
        self.layouts = self.notebook_layout_history();
        self.focus = Focus::Articles;
        Ok(true)
    }

    fn notebook_layout_history(&self) -> crate::model::LayoutHistory {
        let mut history = crate::model::LayoutHistory::new(&self.workspace);
        history.profiles.clone_from(&self.layouts.profiles);
        history
    }

    pub(super) fn persist_notebook_layout(&mut self) -> Result<bool> {
        if self.notebooks.active.is_none() {
            return Ok(false);
        }
        self.layouts
            .observe(crate::model::PanelLayout::capture(&self.workspace));
        let Some((saved, layouts)) = self
            .notebooks
            .layout_snapshot(&self.workspace, &self.layouts)
        else {
            return Ok(false);
        };
        if let Some(writer) = &self.session_writer {
            writer.layout(&saved, layouts)?;
        } else {
            self.store.save_layouts(&saved, layouts)?;
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
        if let Some(book) = &mut self.notebooks.active {
            let limit = area.map(|area| {
                crate::ui::notebook_scroll_limit(&book.view(), area.height)
            });
            book.clamp_viewport(limit);
        }
    }

    pub(super) fn resize_notebook_context(&mut self, width: usize) {
        if let Some(book) = &mut self.notebooks.active {
            book.resize_context(width);
        }
        self.clamp_notebook_scroll();
    }

    /// Poll note search/context workers without blocking input.
    ///
    /// # Errors
    /// Returns read failures while retaining existing previews.
    pub fn poll_notebook(&mut self) -> Result<bool> {
        let changed = self.notebooks.poll()?;
        if changed {
            self.clamp_notebook_scroll();
        }
        Ok(changed)
    }

    /// Whether note search or its selected source context is still pending.
    pub fn notebook_pending(&self) -> bool {
        self.notebooks.pending()
    }
}

#[cfg(test)]
mod tests;

#[derive(Clone)]
struct Origin {
    layout: crate::model::PanelLayout,
    layouts: crate::model::LayoutHistory,
    focus: Focus,
}

/// Active/source-return workspaces and the intact suspended editor share one
/// lifecycle. Storage write admission remains the application's responsibility.
#[derive(Default)]
pub(super) struct NotebookState {
    active: Option<Box<Notebook>>,
    returning: Option<Box<Notebook>>,
    editor: Option<Box<super::Input>>,
    inspector: Option<inspection::Inspector>,
}
