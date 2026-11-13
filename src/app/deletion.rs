//! Article lifecycle actions retain captured identities and support recovery.

use crate::app::commands::routing;

use anyhow::{Result, anyhow};

use super::personal_tasks::{Deletion, DeletionOutcome, LibraryEdit};
use super::{Action, App, Choice, ChoiceAction, Focus, View};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub(super) fn article_delete_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.focus != Focus::Articles
            || self.reader.selecting()
            || !key.modifiers.difference(KeyModifiers::SHIFT).is_empty()
        {
            return Ok(false);
        }
        if key.code == KeyCode::Esc
            && self.pending_article_delete.take().is_some()
        {
            self.status = "Article deletion canceled".into();
            return Ok(true);
        }
        if key.code != KeyCode::Char('d') {
            return Ok(false);
        }
        if key.kind == KeyEventKind::Repeat {
            return Ok(true);
        }
        let Some(article) = self.current().map(|article| article.id) else {
            self.status = "Select an article first".into();
            return Ok(true);
        };
        if self.pending_article_delete.take() == Some(article) {
            self.confirm_article_deletion(article)?;
        } else {
            self.pending_article_delete = Some(article);
            self.status = if *self.nav.view() == View::Trash {
                "d again permanently deletes this article · cannot undo · Esc cancels"
            } else {
                "d again moves this article to Trash · Esc cancels"
            }.into();
        }
        Ok(true)
    }

    fn confirm_article_deletion(&mut self, article: i64) -> Result<()> {
        if *self.nav.view() == View::Trash {
            self.submit_library_edit(LibraryEdit::Delete(Deletion::Purge(
                article,
            )))?;
            self.status = "Article permanently deleted · cannot undo".into();
        } else {
            self.change_article_deleted(article, true)?;
        }
        Ok(())
    }

    pub(super) fn article_command(
        &mut self,
        command: routing::ArticleCommand,
        argument: &str,
    ) -> Result<Action> {
        match command {
            routing::ArticleCommand::DeleteArticle => {
                self.set_selected_deleted(true)?
            }
            routing::ArticleCommand::RestoreArticle => {
                self.set_selected_deleted(false)?
            }
            routing::ArticleCommand::Trash => match argument {
                "" => self.set_view(View::Trash)?,
                "clean" => self.clean_trash()?,
                _ => anyhow::bail!("Usage: :trash [clean]"),
            },
        }
        Ok(Action::None)
    }

    fn clean_trash(&mut self) -> Result<()> {
        self.remove_library_articles(View::Trash)
    }

    pub(super) fn remove_library_articles(&mut self, view: View) -> Result<()> {
        let edit = if view == View::Trash {
            Deletion::CleanTrash
        } else {
            Deletion::View(view)
        };
        self.submit_library_edit(LibraryEdit::Delete(edit))?;
        if self.personal_writes.is_some() {
            self.status = "Library deletion queued".into();
        }
        Ok(())
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
        self.submit_library_edit(LibraryEdit::Delete(Deletion::Trash {
            article,
            deleted,
        }))?;
        if self.nav.retained_article() == Some(article) {
            self.nav.retain_article(None);
        }
        self.status = if deleted {
            "Article moved to Trash · u undo · :trash to restore".into()
        } else {
            "Article restored · u undo".into()
        };
        Ok(())
    }

    pub(super) fn finish_article_deletion(
        &mut self,
        article: i64,
        deleted: bool,
    ) -> Result<()> {
        let status = if deleted {
            "Article moved to Trash · u undo · :trash to restore"
        } else {
            "Article restored · u undo"
        };
        self.finish_article_removal(
            article,
            deleted != (*self.nav.view() == View::Trash),
            status,
        )
    }

    pub(super) fn finish_deletion(
        &mut self,
        result: DeletionOutcome,
    ) -> Result<()> {
        match result {
            DeletionOutcome::Trash { article, deleted } => {
                self.finish_article_deletion(article, deleted)
            }
            DeletionOutcome::Purged(article) => {
                self.finish_article_purge(article)
            }
            DeletionOutcome::Cleaned(count) => {
                self.finish_view_deletion(View::Trash, count,
                    format!("Trash emptied · {count} articles permanently deleted · cannot undo"))
            }
            DeletionOutcome::Trashed { view, count } => self.finish_view_deletion(
                view, count, format!("{count} articles moved to Trash · u undo"),
            ),
        }
    }

    fn finish_article_purge(&mut self, article: i64) -> Result<()> {
        self.finish_article_removal(
            article,
            true,
            "Article permanently deleted · cannot undo",
        )
    }

    fn finish_article_removal(
        &mut self,
        article: i64,
        remove: bool,
        status: &str,
    ) -> Result<()> {
        // Commit acknowledgement owns visibility. Counts and the next source
        // can be expensive; neither is a prerequisite for removing this row.
        let checkpoint = self.save_reading_position();
        if self.nav.retained_article() == Some(article) {
            self.nav.retain_article(None);
        }
        if matches!(self.mode, super::Mode::Normal)
            && !self.reader.selecting()
            && self.notebook().is_none()
        {
            if remove {
                self.remove_deleted_metadata(article);
            }
            if self.current().is_none() {
                self.clear_current();
            }
        }
        self.refresh_deleted_articles()?;
        self.status = status.into();
        checkpoint
    }

    fn finish_view_deletion(
        &mut self,
        view: View,
        count: usize,
        status: String,
    ) -> Result<()> {
        if count != 0 {
            if self.nav.view().same_destination(&view)
                && matches!(self.mode, super::Mode::Normal)
                && self.notebook().is_none()
                && !self.reader.selecting()
            {
                self.nav.clear_deleted_destination();
                self.clear_current();
            }
            self.refresh_deleted_articles()?;
        }
        self.status = status;
        Ok(())
    }

    fn refresh_deleted_articles(&mut self) -> Result<()> {
        if self.navigation_worker.is_none() {
            return self.reload();
        }
        self.nav.invalidate_library();
        self.invalidate_notebook();
        self.refresh_catalog_page();
        self.request_catalog(super::catalog_worker::Scope::Counts);
        Ok(())
    }

    fn remove_deleted_metadata(&mut self, article: i64) {
        self.nav.remove_deleted_metadata(article);
    }

    pub(super) fn deletion_choice(&self) -> Option<Choice> {
        let article = self.current()?;
        let deleted = *self.nav.view() != View::Trash;
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
