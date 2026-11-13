//! Workspace preferences, focus, and reversible layout history.
//!
//! Geometry adapters borrow preferences. Changes enter through domain operations;
//! no mutable workspace or history reference escapes this owner.

mod navigation;
mod preferences;
mod visual;
pub(super) use preferences::Dimension;

use anyhow::{Context, Result, ensure};
use ratatui::layout::Rect;

use crate::model::{
    Dock, LayoutHistory, Pane, PaneLayout, PanelLayout, View, Workspace,
};

/// Geometry inputs borrowed together from one workspace revision.
#[derive(Clone, Copy)]
pub struct LayoutView<'a> {
    /// Persisted preferences used by geometry and rendering.
    pub workspace: &'a Workspace,
    /// Pane receiving keyboard input.
    pub focus: Pane,
    /// Last observed terminal dimensions.
    pub screen: Rect,
}

pub(super) struct LayoutState {
    workspace: Workspace,
    history: LayoutHistory,
    focus: Pane,
    fullscreen_return: Option<Pane>,
    screen: Rect,
}

impl LayoutState {
    pub(super) fn new(workspace: Workspace, history: LayoutHistory) -> Self {
        let focus = if workspace.fullscreen {
            Pane::Reader
        } else {
            Pane::Articles
        };
        Self {
            workspace,
            history,
            focus,
            fullscreen_return: None,
            screen: Rect::default(),
        }
    }

    pub(super) fn view(&self) -> LayoutView<'_> {
        LayoutView {
            workspace: &self.workspace,
            focus: self.focus,
            screen: self.screen,
        }
    }

    pub(super) fn workspace(&self) -> &Workspace {
        &self.workspace
    }

    pub(super) fn history(&self) -> &LayoutHistory {
        &self.history
    }

    pub(super) fn focus(&self) -> Pane {
        self.focus
    }

    /// Activating a pane expands it; fullscreen/Zen cannot hide a nonreader
    /// keyboard target. Source selection remains the reader owner's decision.
    pub(super) fn focus_pane(&mut self, pane: Pane) {
        if pane != Pane::Reader {
            self.workspace.fullscreen = false;
            self.workspace.zen = false;
            self.fullscreen_return = None;
        }
        self.workspace.set_pane_visible(pane, true);
        self.focus = pane;
    }

    pub(super) fn ensure_visible_focus(&mut self) {
        if self.workspace.fullscreen
            || self.workspace.zen && self.workspace.show_reader
        {
            self.focus = Pane::Reader;
        } else if !self.workspace.pane_visible(self.focus) {
            self.focus = if self.workspace.show_reader {
                Pane::Reader
            } else {
                self.workspace
                    .panel_order
                    .into_iter()
                    .find(|pane| self.workspace.pane_visible(*pane))
                    .unwrap_or(Pane::Reader)
            };
        }
    }

    pub(super) fn toggle_fullscreen(&mut self) {
        if self.workspace.fullscreen {
            self.focus = self.fullscreen_return.take().unwrap_or(Pane::Reader);
        } else {
            self.fullscreen_return = Some(self.focus);
            self.workspace.show_reader = true;
            self.focus = Pane::Reader;
        }
        self.workspace.fullscreen = !self.workspace.fullscreen;
        self.ensure_visible_focus();
    }

    pub(super) fn screen(&self) -> Rect {
        self.screen
    }

    pub(super) fn resize(&mut self, screen: Rect) {
        self.screen = screen;
    }

    pub(super) fn remember_fullscreen_focus(&mut self, pane: Option<Pane>) {
        self.fullscreen_return = pane;
    }

    pub(super) fn apply_panel_layout(&mut self, layout: &PanelLayout) {
        layout.apply(&mut self.workspace);
    }

    pub(super) fn reset_columns(&mut self) {
        self.workspace.layout = None;
    }

    pub(super) fn restore_tree(&mut self, tree: PaneLayout) {
        self.workspace.layout = Some(tree);
    }

    pub(super) fn resize_split(&mut self, path: &[bool], percent: u16) -> bool {
        self.workspace
            .layout
            .as_mut()
            .is_some_and(|tree| tree.resize_path(path, percent))
    }

    pub(super) fn dock(
        &mut self,
        pane: Pane,
        target: Pane,
        edge: Dock,
    ) -> Result<()> {
        let tree = self.workspace.layout.clone().unwrap_or_else(|| {
            PaneLayout::columns(&self.workspace.panel_order)
        });
        self.workspace.layout = Some(
            tree.dock(pane, target, edge)
                .context("Choose two different panes in a valid layout")?,
        );
        self.show_pane(pane, true);
        self.show_pane(target, true);
        Ok(())
    }

    pub(super) fn show_pane(&mut self, pane: Pane, visible: bool) {
        self.workspace.set_pane_visible(pane, visible);
    }

    pub(super) fn resize_pane(&mut self, pane: Pane, delta: i32) {
        self.workspace.resize_pane(pane, delta);
    }

    pub(super) fn reorder_pane(&mut self, pane: Pane, target: Pane) {
        self.workspace.panel_order =
            super::panels::reordered(self.workspace.panel_order, pane, target);
    }

    pub(super) fn restore_widths(
        &mut self,
        library: u16,
        articles: u16,
        annotations: u16,
    ) {
        self.workspace.library_width = library;
        self.workspace.article_width = articles;
        self.workspace.annotation_width = annotations;
    }

    pub(super) fn observe(&mut self) {
        self.history.observe(PanelLayout::capture(&self.workspace));
    }

    pub(super) fn restore_history(
        &mut self,
        redo: bool,
    ) -> Option<PanelLayout> {
        if redo {
            self.history.redo()
        } else {
            self.history.undo()
        }
    }

    pub(super) fn save_profile(&mut self, name: &str) -> Result<()> {
        ensure!(
            self.history.profiles.contains_key(name)
                || self.history.profiles.len() < 32,
            "At most 32 workspaces"
        );
        self.history
            .profiles
            .insert(name.into(), PanelLayout::capture(&self.workspace));
        Ok(())
    }

    pub(super) fn load_profile(&mut self, name: &str) -> Result<()> {
        let layout = self
            .history
            .profiles
            .get(name)
            .context("Workspace not found")?;
        layout.apply(&mut self.workspace);
        Ok(())
    }

    pub(super) fn delete_profile(&mut self, name: &str) -> Result<()> {
        ensure!(
            self.history.profiles.remove(name).is_some(),
            "Workspace not found"
        );
        Ok(())
    }

    pub(super) fn install_profiles(
        &mut self,
        profiles: std::collections::BTreeMap<String, PanelLayout>,
    ) {
        self.history.profiles = profiles;
    }

    pub(super) fn enter_notebook(&mut self, initialize: bool) {
        if initialize {
            self.workspace.fullscreen = false;
            self.workspace.zen = false;
            self.workspace.show_library = true;
            self.workspace.show_articles = true;
            self.workspace.show_reader = true;
            self.workspace.show_annotations = false;
        }
        if let Some(layout) = &self.workspace.notebook_layout {
            let layout = layout.clone();
            layout.apply(&mut self.workspace);
        }
        let mut history = LayoutHistory::new(&self.workspace);
        history.profiles.clone_from(&self.history.profiles);
        self.history = history;
        self.focus = Pane::Articles;
    }

    pub(super) fn leave_notebook(
        &mut self,
        layout: &PanelLayout,
        history: LayoutHistory,
        focus: Pane,
    ) {
        self.workspace.notebook_layout =
            Some(PanelLayout::capture(&self.workspace));
        layout.apply(&mut self.workspace);
        self.history = history;
        self.focus = focus;
    }

    pub(super) fn remember_session(
        &mut self,
        article: Option<i64>,
        view: View,
        page_offset: usize,
    ) {
        self.workspace.article = article;
        self.workspace.view = view;
        self.workspace.page_offset = page_offset;
    }

    pub(super) fn end_visit(&mut self, at: i64) {
        self.workspace.last_visit = at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> LayoutState {
        let workspace = Workspace::default();
        LayoutState::new(workspace.clone(), LayoutHistory::new(&workspace))
    }

    #[test]
    fn fullscreen_returns_to_the_captured_focus_and_reconciles_hidden_panes() {
        let mut state = state();
        state.focus_pane(Pane::Library);
        state.toggle_fullscreen();
        assert_eq!(state.focus(), Pane::Reader);
        state.toggle_fullscreen();
        assert_eq!(state.focus(), Pane::Library);
        state.show_pane(Pane::Library, false);
        state.ensure_visible_focus();
        assert_eq!(state.focus(), Pane::Reader);
    }

    #[test]
    fn activating_a_pane_cannot_leave_keyboard_focus_hidden_by_fullscreen() {
        let mut state = state();
        state.toggle_fullscreen();
        state.focus_pane(Pane::Annotations);
        assert!(!state.workspace().fullscreen);
        assert!(state.workspace().show_annotations);
        assert_eq!(state.focus(), Pane::Annotations);
    }

    #[test]
    fn rejected_docking_preserves_the_entire_observed_layout() {
        let mut state = state();
        let before = PanelLayout::capture(state.workspace());
        assert!(state.dock(Pane::Reader, Pane::Reader, Dock::Above).is_err());
        assert_eq!(PanelLayout::capture(state.workspace()), before);
        assert_eq!(state.history().current, before);
    }
}
