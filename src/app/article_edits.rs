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
    reconcile_required: bool,
}

pub(super) enum EditCompletion {
    Pending,
    Reconcile(super::catalog_worker::Scope),
}

struct CapturedEdit {
    articles: BTreeSet<i64>,
    edit: crate::model::BulkEdit,
}

impl ArticleEdits {
    pub(super) fn capture(
        &mut self,
        ticket: u64,
        articles: BTreeSet<i64>,
        edit: crate::model::BulkEdit,
    ) {
        self.pending.insert(ticket, CapturedEdit { articles, edit });
    }

    pub(super) fn removing(&self, target: &View) -> bool {
        self.removals
            .values()
            .any(|pending| pending.same_destination(target))
    }

    pub(super) fn capture_removal(&mut self, ticket: u64, target: View) {
        self.removals.insert(ticket, target);
    }

    pub(super) fn finish_removal(&mut self, ticket: u64) {
        self.removals.remove(&ticket);
    }

    pub(super) fn reconcile_required(&self) -> bool {
        self.reconcile_required
    }

    pub(super) fn reconciled(&mut self) {
        self.reconcile_required = false;
    }

    pub(super) fn complete(
        &mut self,
        ticket: u64,
        failed: bool,
    ) -> Option<EditCompletion> {
        let pending = self.pending.remove(&ticket)?;
        self.reconcile_required |= failed;
        self.failed |= failed;
        self.changes = self
            .changes
            .union(crate::search::Changes::bulk(&pending.edit));
        if !self.pending.is_empty() {
            return Some(EditCompletion::Pending);
        }
        let scope = if self.failed {
            super::catalog_worker::Scope::Library
        } else {
            super::catalog_worker::Scope::Changes(self.changes)
        };
        self.failed = false;
        self.changes = crate::search::Changes::default();
        Some(EditCompletion::Reconcile(scope))
    }

    pub(super) fn overlays(
        &self,
    ) -> impl Iterator<Item = (&BTreeSet<i64>, &crate::model::BulkEdit)> {
        self.pending
            .values()
            .map(|pending| (&pending.articles, &pending.edit))
    }
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
        if self.personal.enabled() {
            let ticket = self.personal.submit(Task::Articles {
                articles: articles.clone(),
                edit: edit.clone(),
            })?;
            self.personal.article_editing().capture(
                ticket,
                articles.clone(),
                edit.clone(),
            );
        } else {
            super::personal_tasks::operation::article_edit(
                &self.store,
                articles.clone(),
                edit.clone(),
            )?;
        }
        self.nav.overlay_edit(&articles, &edit);
        Ok(())
    }

    pub(super) fn submit_library_edit(
        &mut self,
        edit: LibraryEdit,
    ) -> Result<()> {
        if self.personal.enabled() {
            let target = edit.removal_target();
            if target
                .as_ref()
                .is_some_and(|target| self.personal.articles().removing(target))
            {
                self.interaction
                    .report("Library removal already queued".into());
                return Ok(());
            }
            let ticket = self.personal.submit(Task::Library(edit))?;
            if let Some(target) = target {
                self.personal
                    .article_editing()
                    .capture_removal(ticket, target);
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
        self.personal.article_editing().finish_removal(ticket);
    }

    pub(super) fn complete_article_edit(
        &mut self,
        ticket: u64,
        failed: bool,
    ) -> bool {
        let Some(scope) =
            self.personal.article_editing().complete(ticket, failed)
        else {
            return false;
        };
        self.nav.invalidate_library();
        if let EditCompletion::Reconcile(scope) = scope {
            self.request_catalog(scope);
        }
        true
    }

    pub(super) fn overlay_article_edits(&mut self) {
        for (articles, edit) in self.personal.articles().overlays() {
            self.nav.overlay_edit(articles, edit);
        }
    }
}

pub(super) fn overlay(
    article: &mut crate::model::Article,
    edit: &crate::model::BulkEdit,
) {
    match edit {
        crate::model::BulkEdit::Unread(value) => article.unread = *value,
        crate::model::BulkEdit::Favorite(value) => article.starred = *value,
        crate::model::BulkEdit::Queue(value) => article.queued = *value,
        _ => {}
    }
}
