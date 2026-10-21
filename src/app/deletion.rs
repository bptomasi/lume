//! Article lifecycle actions retain captured identities and support recovery.

use anyhow::{Result, anyhow};

use super::{Action, App, Choice, ChoiceAction, View};

impl App {
    pub(super) fn article_command(
        &mut self,
        verb: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "delete-article" => self.set_selected_deleted(true)?,
            "restore-article" => self.set_selected_deleted(false)?,
            "trash" => self.set_view(View::Trash)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    pub(super) fn set_selected_deleted(&mut self, deleted: bool) -> Result<()> {
        let article = self
            .current()
            .map(|article| article.id)
            .ok_or_else(|| anyhow!("Select an article first"))?;
        self.change_article_deleted(article, deleted)
    }

    pub(super) fn change_article_deleted(
        &mut self,
        article: i64,
        deleted: bool,
    ) -> Result<()> {
        self.submit_library_edit(
            super::personal_tasks::LibraryEdit::Deleted { article, deleted },
        )?;
        if self.retained_article == Some(article) {
            self.retained_article = None;
        }
        if self.personal_writes.is_none() {
            self.reload()?;
        }
        self.status = if deleted {
            "Article moved to Trash · u undo · :trash to restore".into()
        } else {
            "Article restored · u undo".into()
        };
        Ok(())
    }

    pub(super) fn deletion_choice(&self) -> Option<Choice> {
        let article = self.current()?;
        let deleted = self.view != View::Trash;
        Some(Choice {
            title: if deleted {
                "Delete article".into()
            } else {
                "Restore article".into()
            },
            detail: format!("{} · {} · u undoes", article.title, article.feed),
            action: ChoiceAction::SetDeleted {
                article: article.id,
                deleted,
            },
        })
    }
}
