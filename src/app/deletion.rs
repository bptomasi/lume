//! Article lifecycle actions retain captured identities and support recovery.

use anyhow::{Result, anyhow};

use super::personal_tasks::{Deletion, DeletionOutcome, LibraryEdit};
use super::{Action, App, Choice, ChoiceAction, Focus, View};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

impl App {
    pub(super) fn article_delete_key(&mut self, key: KeyEvent) -> Result<bool> {
        if self.focus != Focus::Articles
            || self.selection.is_some()
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
            self.status = if self.view == View::Trash {
                "d again permanently deletes this article · cannot undo · Esc cancels"
            } else {
                "d again moves this article to Trash · Esc cancels"
            }.into();
        }
        Ok(true)
    }

    fn confirm_article_deletion(&mut self, article: i64) -> Result<()> {
        if self.view == View::Trash {
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
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "delete-article" => self.set_selected_deleted(true)?,
            "restore-article" => self.set_selected_deleted(false)?,
            "trash" => match argument {
                "" => self.set_view(View::Trash)?,
                "clean" => self.clean_trash()?,
                _ => anyhow::bail!("Usage: :trash [clean]"),
            },
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn clean_trash(&mut self) -> Result<()> {
        self.submit_library_edit(LibraryEdit::Delete(Deletion::CleanTrash))?;
        self.status = "Trash cleanup requested · permanent deletion".into();
        if self.personal_writes.is_none() {
            self.status = "Trash emptied · cannot undo".into();
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
        if self.retained_article == Some(article) {
            self.retained_article = None;
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
            deleted != (self.view == View::Trash),
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
                self.reload()?;
                self.status = format!(
                    "Trash emptied · {count} articles permanently deleted · cannot undo"
                );
                Ok(())
            }
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
        self.library_revision = self.library_revision.wrapping_add(1);
        self.invalidate_notebook();
        if self.retained_article == Some(article) {
            self.retained_article = None;
        }
        if matches!(self.mode, super::Mode::Normal)
            && self.selection.is_none()
            && self.notebook.is_none()
        {
            if remove {
                self.remove_deleted_metadata(article);
            }
            if self.current().is_none() {
                self.clear_current();
            }
        }
        self.refresh_catalog_page();
        self.request_catalog(super::catalog_worker::Scope::Counts);
        self.status = status.into();
        checkpoint
    }

    fn remove_deleted_metadata(&mut self, article: i64) {
        let selected = self.current().map(|item| item.id);
        let previous = self.library.articles.len();
        self.library.articles.retain(|item| item.id != article);
        if self.library.articles.len() == previous {
            return;
        }
        self.visible = (0..self.library.articles.len()).collect();
        self.selected = selected
            .filter(|id| *id != article)
            .and_then(|id| {
                self.library.articles.iter().position(|item| item.id == id)
            })
            .unwrap_or_else(|| {
                self.selected.min(self.visible.len().saturating_sub(1))
            });
        // Grouped results may replace the deleted representative with another
        // occurrence. Let the authoritative page determine its group count.
        if self.view != View::Grouped {
            self.page_total = self.page_total.saturating_sub(1);
        }
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
