//! Profiles and layout history commit atomically with the active workspace.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use crate::model::{LayoutHistory, Workspace};

use super::Store;

impl Store {
    /// Load bounded profiles/history, or seed defaults for an existing library.
    ///
    /// # Errors
    /// Rejects malformed history and invalid saved layout dimensions/topology.
    pub fn layouts(&self, workspace: &Workspace) -> Result<LayoutHistory> {
        let json: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'layouts'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let layouts = json
            .map(|json| serde_json::from_str(&json))
            .transpose()?
            .unwrap_or_else(|| LayoutHistory::new(workspace));
        self.validate_layouts(&layouts)?;
        Ok(layouts)
    }

    /// Save workspace and its layout journal in one SQLite transaction.
    ///
    /// # Errors
    /// Returns validation, serialization or SQLite failures without partial saves.
    pub fn save_layouts(
        &self,
        workspace: &Workspace,
        layouts: &LayoutHistory,
    ) -> Result<()> {
        self.validate_layouts(layouts)?;
        ensure!(
            layouts.current == crate::model::PanelLayout::capture(workspace),
            "Layout journal does not match active workspace"
        );
        let transaction = self.connection.unchecked_transaction()?;
        self.save_workspace(workspace)?;
        self.connection.execute(
            include_str!("sql/insert_layout_history.sql"),
            [serde_json::to_string(layouts)?],
        )?;
        transaction.commit()?;
        Ok(())
    }

    fn validate_layouts(&self, layouts: &LayoutHistory) -> Result<()> {
        ensure!(layouts.profiles.len() <= 32, "At most 32 workspaces");
        ensure!(
            layouts.past.len() <= 32 && layouts.future.len() <= 32,
            "Layout history exceeds 32 entries"
        );
        for name in layouts.profiles.keys() {
            ensure!(
                valid_name(name),
                "Workspace names require 1–64 plain characters"
            );
        }
        for layout in layouts
            .profiles
            .values()
            .chain(&layouts.past)
            .chain(&layouts.future)
            .chain(std::iter::once(&layouts.current))
        {
            let mut workspace = Workspace::default();
            layout.apply(&mut workspace);
            super::workspace::validate_dimensions(&workspace)?;
        }
        Ok(())
    }
}

pub(crate) fn valid_name(name: &str) -> bool {
    !name.trim().is_empty()
        && name.len() <= 256
        && name.chars().count() <= 64
        && name.trim() == name
        && !name.chars().any(char::is_control)
}
