//! Two explicit key presses capture a sidebar definition, never an article.
use super::{App, Focus, View, personal_tasks::LibraryEdit};
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub(super) fn library_delete_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.focus != Focus::Library
            || !key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
        {
            return Ok(false);
        }
        if key.code == KeyCode::Esc
            && self.pending_destination_delete.take().is_some()
        {
            self.status = "Library removal canceled".into();
            return Ok(true);
        }
        if key.code != KeyCode::Char('d') {
            return Ok(false);
        }
        if key.kind == KeyEventKind::Repeat {
            return Ok(true);
        }
        let target = self
            .navigation
            .get(self.nav_selected)
            .context("Select a feed or smart list")?
            .view
            .clone();
        if !matches!(target, View::Feed(_) | View::Smart(_)) {
            self.status = "Select a feed or smart list to remove".into();
            return Ok(true);
        }
        if self.pending_destination_delete.take().as_ref() == Some(&target) {
            self.remove_destination(target)?;
        } else {
            self.pending_destination_delete = Some(target);
            self.status =
                "d again removes this feed or smart list · Esc cancels".into();
        }
        Ok(true)
    }

    pub(super) fn finish_definition_removal(&mut self) -> Result<()> {
        self.reload()?;
        self.status =
            "Library definition removed · articles and notes kept".into();
        Ok(())
    }

    pub(super) fn remove_destination(&mut self, target: View) -> Result<()> {
        let (edit, label) = match target {
            View::Feed(id) => (LibraryEdit::Unsubscribe(id), "Feed"),
            View::Smart(id) => (LibraryEdit::DeleteSmart(id), "Smart list"),
            _ => anyhow::bail!("Select a feed or smart list"),
        };
        let active = target.same_destination(&self.view);
        self.save_reading_position()?;
        self.submit_library_edit(edit)?;
        self.workspace
            .pins
            .retain(|view| !view.same_destination(&target));
        self.workspace
            .navigation_order
            .retain(|view| !view.same_destination(&target));
        if active {
            self.set_view(View::All)?;
        }
        self.persist_preferences()?;
        self.status = format!("{label} removed · articles and notes kept");
        Ok(())
    }
}
