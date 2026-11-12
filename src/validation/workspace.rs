//! Bounds shared by declarative preferences and durable workspace writes.

use anyhow::{Result, ensure};

use crate::model::Workspace;

pub(crate) fn validate_panel_layout(workspace: &Workspace) -> Result<()> {
    ensure!(
        workspace.valid_panel_order()
            && workspace
                .layout
                .as_ref()
                .is_none_or(crate::model::PaneLayout::valid),
        "Pane order must contain every pane exactly once"
    );
    ensure!(
        (24..=80).contains(&workspace.annotation_width),
        "Annotations width must be 24–80"
    );
    Ok(())
}

pub(crate) fn validate_workspace_dimensions(
    workspace: &Workspace,
) -> Result<()> {
    workspace.auto_read.validate()?;
    validate_panel_layout(workspace)?;
    if let Some(layout) = &workspace.notebook_layout {
        let mut notebook = Workspace::default();
        layout.apply(&mut notebook);
        validate_workspace_dimensions(&notebook)?;
    }
    ensure!(
        ["lume", "nord", "gruvbox"].contains(&workspace.theme.as_str()),
        "Unsupported theme"
    );
    ensure!(
        workspace.keybindings.len() <= 128,
        "At most 128 custom key bindings"
    );
    ensure!(
        (40..=140).contains(&workspace.reading_width),
        "Reading width must be 40–140"
    );
    ensure!(
        (40..=140).contains(&workspace.fullscreen_width),
        "Fullscreen width must be 40–140"
    );
    ensure!(
        (16..=48).contains(&workspace.library_width),
        "Sidebar width must be 16–48"
    );
    ensure!(
        (24..=80).contains(&workspace.article_width),
        "Headlines width must be 24–80"
    );
    Ok(())
}
