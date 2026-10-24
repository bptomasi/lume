//! Optimistic article flags share the ordered personal-write lane and undo.
//!
//! Admission precedes presentation. Failed writes trigger an authoritative
//! catalog read; pending newer intentions overlay older database snapshots.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::Result;

use super::{
    App, View,
    personal_tasks::{LibraryEdit, Outcome, Task},
};

#[derive(Default)]
pub(super) struct ArticleEdits {
    pending: std::collections::BTreeMap<u64, CapturedEdit>,
    removals: BTreeMap<u64, View>,
    changes: crate::search::Changes,
    failed: bool,
    pub(super) reconcile_required: bool,
}

struct CapturedEdit {
    articles: BTreeSet<i64>,
    edit: crate::model::BulkEdit,
}

impl App {
    pub(super) fn submit_read_state(
        &mut self,
        articles: BTreeSet<i64>,
        unread: bool,
    ) -> Result<()> {
        self.submit_article_edit(
            articles,
            crate::model::BulkEdit::Unread(unread),
        )
    }

    pub(super) fn submit_article_edit(
        &mut self,
        articles: BTreeSet<i64>,
        edit: crate::model::BulkEdit,
    ) -> Result<()> {
        anyhow::ensure!(
            !articles.is_empty() && articles.len() <= 1000,
            "Select 1–1,000 articles"
        );
        if let Some(writer) = &mut self.personal_writes {
            let ticket = writer.submit(Task::Articles {
                articles: articles.clone(),
                edit: edit.clone(),
            })?;
            self.article_edits.pending.insert(
                ticket,
                CapturedEdit {
                    articles: articles.clone(),
                    edit: edit.clone(),
                },
            );
        } else {
            super::personal_tasks::operation::article_edit(
                &self.store,
                articles.clone(),
                edit.clone(),
            )?;
        }
        for article in &mut self.library.articles {
            if articles.contains(&article.id) {
                overlay(article, &edit);
            }
        }
        Ok(())
    }

    pub(super) fn submit_library_edit(
        &mut self,
        edit: LibraryEdit,
    ) -> Result<()> {
        if let Some(writer) = &mut self.personal_writes {
            let target = edit.removal_target();
            if target.as_ref().is_some_and(|target| {
                self.article_edits
                    .removals
                    .values()
                    .any(|pending| pending.same_destination(target))
            }) {
                self.status = "Library removal already queued".into();
                return Ok(());
            }
            let ticket = writer.submit(Task::Library(edit))?;
            if let Some(target) = target {
                self.article_edits.removals.insert(ticket, target);
            }
        } else {
            let outcome = super::personal_tasks::operation::library_edit(
                &self.store,
                edit,
            )?;
            match outcome {
                Outcome::Deleted(result) => self.finish_deletion(result)?,
                Outcome::DefinitionRemoved(view) => {
                    self.finish_definition_removal(view)?
                }
                _ => self.reload()?,
            }
        }
        Ok(())
    }

    pub(super) fn finish_library_edit(&mut self, ticket: u64) {
        self.article_edits.removals.remove(&ticket);
    }

    pub(super) fn complete_article_edit(
        &mut self,
        ticket: u64,
        failed: bool,
    ) -> bool {
        let Some(pending) = self.article_edits.pending.remove(&ticket) else {
            return false;
        };
        self.library_revision = self.library_revision.wrapping_add(1);
        self.article_edits.reconcile_required |= failed;
        self.article_edits.failed |= failed;
        self.article_edits.changes = self
            .article_edits
            .changes
            .union(crate::search::Changes::bulk(&pending.edit));
        if self.article_edits.pending.is_empty() {
            let scope = if self.article_edits.failed {
                super::catalog_worker::Scope::Library
            } else {
                super::catalog_worker::Scope::Changes(
                    self.article_edits.changes,
                )
            };
            self.article_edits.failed = false;
            self.article_edits.changes = crate::search::Changes::default();
            self.request_catalog(scope);
        }
        true
    }

    pub(super) fn overlay_article_edits(&mut self) {
        for pending in self.article_edits.pending.values() {
            for article in &mut self.library.articles {
                if pending.articles.contains(&article.id) {
                    overlay(article, &pending.edit);
                }
            }
        }
    }
}

fn overlay(article: &mut crate::model::Article, edit: &crate::model::BulkEdit) {
    match edit {
        crate::model::BulkEdit::Unread(value) => article.unread = *value,
        crate::model::BulkEdit::Favorite(value) => article.starred = *value,
        crate::model::BulkEdit::Queue(value) => article.queued = *value,
        _ => {}
    }
}
