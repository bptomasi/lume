//! Validated preference changes and captured preview restoration.

use anyhow::Result;

use crate::model::{
    AutoReadSettings, ContentSource, ImagePreference, Workspace,
};

use super::LayoutState;

impl LayoutState {
    pub(in crate::app) fn restore_preview(&mut self, workspace: Workspace) {
        self.workspace = workspace;
    }

    pub(in crate::app) fn apply_ui(
        &mut self,
        ui: &crate::config::Ui,
    ) -> Result<()> {
        let mut candidate = self.workspace.clone();
        ui.apply(&mut candidate);
        crate::validation::validate_workspace_dimensions(&candidate)?;
        self.workspace = candidate;
        self.ensure_visible_focus();
        Ok(())
    }

    pub(in crate::app) fn advance_appearance(&mut self) {
        self.workspace.appearance_revision =
            self.workspace.appearance_revision.saturating_add(1);
    }

    pub(in crate::app) fn select_source(&mut self, source: ContentSource) {
        self.workspace.content_source = source;
    }

    pub(in crate::app) fn set_fullscreen(&mut self, enabled: bool) {
        self.workspace.fullscreen = enabled;
    }

    pub(in crate::app) fn set_zen(&mut self, enabled: bool) {
        self.workspace.zen = enabled;
    }

    pub(in crate::app) fn set_compact(&mut self, enabled: bool) {
        self.workspace.compact = enabled;
    }

    pub(in crate::app) fn set_mouse(&mut self, enabled: bool) {
        self.workspace.mouse = enabled;
    }

    pub(in crate::app) fn set_theme(&mut self, theme: &str) -> Result<()> {
        anyhow::ensure!(
            ["lume", "nord", "gruvbox"].contains(&theme),
            "Use :theme lume|nord|gruvbox"
        );
        self.workspace.theme = theme.into();
        Ok(())
    }

    pub(in crate::app) fn set_justification(&mut self, enabled: bool) {
        self.workspace.justify = enabled;
    }

    pub(in crate::app) fn set_formulas(&mut self, enabled: bool) {
        self.workspace.latex_formulas = enabled;
    }

    pub(in crate::app) fn set_formula_graphics(&mut self, enabled: bool) {
        self.workspace.latex_graphics = enabled;
    }

    pub(in crate::app) fn set_auto_read(
        &mut self,
        policy: AutoReadSettings,
    ) -> Result<()> {
        policy.validate()?;
        self.workspace.auto_read = policy;
        Ok(())
    }

    pub(in crate::app) fn enable_images(&mut self, enabled: bool) {
        self.workspace.images.enabled = enabled;
    }

    pub(in crate::app) fn set_image_protocol(
        &mut self,
        protocol: ImagePreference,
    ) {
        self.workspace.images.protocol = protocol;
    }

    pub(in crate::app) fn unbind(&mut self, key: &str) {
        self.workspace.keybindings.remove(key);
    }

    pub(in crate::app) fn bind(
        &mut self,
        key: &str,
        command: &str,
    ) -> Result<()> {
        anyhow::ensure!(
            self.workspace.keybindings.contains_key(key)
                || self.workspace.keybindings.len() < 128,
            "At most 128 custom key bindings"
        );
        crate::validation::validate_binding(key, command)?;
        self.workspace
            .keybindings
            .insert(key.into(), command.into());
        Ok(())
    }

    pub(in crate::app) fn preview_visual(
        &mut self,
        field: usize,
        forward: bool,
    ) {
        super::visual::change_visual(&mut self.workspace, field, forward);
    }

    pub(in crate::app) fn preview_value(
        &mut self,
        field: usize,
        value: &str,
    ) -> Result<()> {
        super::visual::apply_value(&mut self.workspace, field, value)
    }
}

/// Independently bounded prose or pane width.
pub(in crate::app) enum Dimension {
    Reading,
    Fullscreen,
    Library,
    Articles,
}

impl LayoutState {
    pub(in crate::app) fn set_dimension(
        &mut self,
        dimension: Dimension,
        width: u16,
    ) -> Result<()> {
        let (label, minimum, maximum) = dimension.bounds();
        anyhow::ensure!(
            (minimum..=maximum).contains(&width),
            "{label} width must be {minimum}–{maximum}"
        );
        match dimension {
            Dimension::Reading => self.workspace.reading_width = width,
            Dimension::Fullscreen => self.workspace.fullscreen_width = width,
            Dimension::Library => self.workspace.library_width = width,
            Dimension::Articles => self.workspace.article_width = width,
        }
        Ok(())
    }
}

impl Dimension {
    fn bounds(&self) -> (&'static str, u16, u16) {
        match self {
            Self::Reading => ("Reading", 40, 140),
            Self::Fullscreen => ("Fullscreen", 40, 140),
            Self::Library => ("Sidebar", 16, 48),
            Self::Articles => ("Headlines", 24, 80),
        }
    }
}
