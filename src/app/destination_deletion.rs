//! Two explicit key presses capture a sidebar definition, never an article.
use super::{App, Focus, View, personal_tasks::LibraryEdit};
use anyhow::Result;
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
            .map(|item| item.view.clone())
            .filter(deletable);
        let Some(target) = target else {
            self.status =
                "Select a Library view, feed, server or smart list".into();
            return Ok(true);
        };
        if self.pending_destination_delete.take().as_ref() == Some(&target) {
            self.remove_destination(target)?;
        } else {
            self.status = deletion_prompt(&target).into();
            self.pending_destination_delete = Some(target);
        }
        Ok(true)
    }

    pub(super) fn remove_destination(&mut self, target: View) -> Result<()> {
        if target.can_trash_all() || target == View::Trash {
            return self.remove_library_articles(target);
        }
        let edit = match target {
            View::Feed(id) => LibraryEdit::Unsubscribe(id),
            View::Server(id) => LibraryEdit::RemoveServer(id),
            View::Smart(id) => LibraryEdit::DeleteSmart(id),
            View::Collection(id) => LibraryEdit::DeleteFolder(id),
            _ => anyhow::bail!("Select a feed, server or smart list"),
        };
        self.save_reading_position()?;
        self.submit_library_edit(edit)?;
        if self.personal_writes.is_some() {
            self.status = "Library removal queued".into();
        }
        Ok(())
    }

    pub(super) fn finish_definition_removal(
        &mut self,
        target: View,
    ) -> Result<()> {
        // Layout preferences change only after the captured removal commits.
        // A rejected deletion must retain its pins, order and active view.
        let active = target.same_destination(&self.view);
        self.workspace
            .pins
            .retain(|view| !view.same_destination(&target));
        self.workspace
            .navigation_order
            .retain(|view| !view.same_destination(&target));
        self.reload()?;
        if active && self.view != View::All {
            self.set_view(View::All)?;
        }
        self.persist_preferences()?;
        self.status = match target {
            View::Smart(_) => "Smart list removed · all articles kept".into(),
            View::Collection(_) => {
                "Folder removed · all articles kept · u undo".into()
            }
            _ => "Source removed · saved articles and notes kept".into(),
        };
        Ok(())
    }
}

fn deletable(view: &View) -> bool {
    view.can_trash_all()
        || matches!(
            view,
            View::Trash
                | View::Feed(_)
                | View::Smart(_)
                | View::Server(_)
                | View::Collection(_)
        )
}

fn deletion_prompt(view: &View) -> &'static str {
    match view {
        View::Trash => {
            "d again permanently empties Trash · cannot undo · Esc cancels"
        }
        View::Feed(_) | View::Server(_) => {
            "d again removes source + unsaved articles · cannot undo · Esc cancels"
        }
        View::Smart(_) => "d again removes this smart list only · Esc cancels",
        View::Collection(_) => {
            "d again removes this folder only · articles kept · u undo · Esc cancels"
        }
        _ => "d again sends this entire view to Trash · u undo · Esc cancels",
    }
}
