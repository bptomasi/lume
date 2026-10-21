//! Workspace preferences, progress, reading queue and saved search contracts.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use crate::model::{SmartFilter, SmartList, SmartListRename, Workspace};

use super::Store;

impl Store {
    pub(super) fn smart_filter(&self, id: i64) -> Result<SmartFilter> {
        let definition: String = self.connection.query_row(
            "SELECT definition FROM smart_lists WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(serde_json::from_str(&definition)?)
    }

    /// Read preferences or defaults for a new library.
    ///
    /// # Errors
    /// Returns SQLite or invalid stored JSON errors.
    pub fn workspace(&self) -> Result<Workspace> {
        let value: Option<String> = self
            .connection
            .query_row(
                "SELECT value FROM settings WHERE key = 'workspace'",
                [],
                |row| row.get(0),
            )
            .optional()?;
        let workspace = value
            .map(|value| serde_json::from_str(&value))
            .transpose()?
            .unwrap_or_default();
        validate_panel_layout(&workspace)?;
        Ok(workspace)
    }

    /// Persist a validated layout and last selected destination.
    ///
    /// # Errors
    /// Returns invalid dimensions, JSON or SQLite errors.
    pub fn save_workspace(&self, workspace: &Workspace) -> Result<()> {
        validate_dimensions(workspace)?;
        self.connection.execute(
            r#"
            INSERT INTO settings (key, value) VALUES ('workspace', ?1)
            ON CONFLICT (key) DO UPDATE SET value = excluded.value
            "#,
            [serde_json::to_string(workspace)?],
        )?;
        Ok(())
    }

    /// Restore an article's source-byte reading anchor.
    ///
    /// # Errors
    /// Returns SQLite errors or a missing article.
    pub fn progress(&self, article: i64) -> Result<usize> {
        Ok(self.connection.query_row(
            "SELECT progress FROM articles WHERE id = ?1",
            [article],
            |row| row.get(0),
        )?)
    }

    /// Save progress without changing read state or publisher content.
    ///
    /// # Errors
    /// Returns integer conversion or SQLite errors.
    pub fn save_progress(&self, article: i64, line: usize) -> Result<()> {
        self.connection.execute(
            "UPDATE articles SET progress = ?1 WHERE id = ?2",
            (i64::try_from(line)?, article),
        )?;
        Ok(())
    }

    /// Set explicit queue membership, independently of read/favorite state.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn set_queued(&self, article: i64, queued: bool) -> Result<()> {
        self.set_flag(article, "queued", queued)
    }

    /// Create or update a named smart list with typed, validated criteria.
    ///
    /// # Errors
    /// Returns invalid criteria, duplicate names or SQLite errors.
    pub fn save_smart_list(
        &self,
        name: &str,
        filter: &SmartFilter,
    ) -> Result<()> {
        let name = super::annotations::collection_name(name)?;
        ensure!(filter.text.len() <= 4096, "Smart-list text exceeds 4 KiB");
        crate::search::parse(&filter.text)?;
        ensure!(filter.feeds.len() <= 128, "Source group exceeds 128 feeds");
        ensure!(
            filter.feeds.iter().all(|id| *id > 0),
            "Invalid source identity"
        );
        ensure!(
            filter.days.is_none_or(|days| days > 0),
            "Days must be positive"
        );
        self.connection.execute(
            r#"
            INSERT INTO smart_lists (name, definition) VALUES (?1, ?2)
            ON CONFLICT (name) DO UPDATE
            SET definition = excluded.definition, enabled = 1
            "#,
            (name, serde_json::to_string(filter)?),
        )?;
        Ok(())
    }

    /// Read saved search criteria.
    ///
    /// # Errors
    /// Returns SQLite or invalid stored JSON errors.
    pub fn smart_lists(&self) -> Result<Vec<SmartList>> {
        let mut statement = self.connection.prepare(
            r#"
            SELECT id, name, definition FROM smart_lists
            WHERE enabled = 1
            ORDER BY name COLLATE NOCASE
            "#,
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        rows.map(|row| {
            let (id, name, definition) = row?;
            Ok(SmartList {
                id,
                name,
                filter: serde_json::from_str(&definition)?,
            })
        })
        .collect()
    }

    /// Delete a saved query without affecting articles.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn delete_smart_list(&self, id: i64) -> Result<()> {
        self.connection
            .execute("DELETE FROM smart_lists WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Rename up to 128 saved searches atomically, retaining IDs and criteria.
    ///
    /// # Errors
    /// Rejects invalid names, missing identities and destination conflicts;
    /// every earlier rename rolls back when any item fails.
    pub fn rename_smart_lists(&self, names: &[SmartListRename]) -> Result<()> {
        ensure!(names.len() <= 128, "Rename batch exceeds 128 lists");
        let transaction = self.write_transaction()?;
        for change in names {
            let name = super::annotations::collection_name(&change.name)?;
            let changed = transaction.execute(
                include_str!("sql/update_smart_list_name.sql"),
                (name, change.id, &change.expected_name),
            )?;
            ensure!(
                changed == 1,
                "Saved search changed or no longer exists: {}",
                change.id
            );
        }
        transaction.commit()?;
        Ok(())
    }
}

fn validate_panel_layout(workspace: &Workspace) -> Result<()> {
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

pub(crate) fn validate_dimensions(workspace: &Workspace) -> Result<()> {
    validate_panel_layout(workspace)?;
    if let Some(layout) = &workspace.notebook_layout {
        let mut notebook = Workspace::default();
        layout.apply(&mut notebook);
        validate_dimensions(&notebook)?;
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
