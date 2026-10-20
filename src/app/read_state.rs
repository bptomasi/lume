//! Optimistic read flags use the ordered personal-write lane, including undo.
//!
//! Admission precedes presentation. Failed writes trigger an authoritative
//! catalog read; pending newer intentions overlay older database snapshots.

use std::collections::BTreeSet;

use anyhow::Result;

use super::{App, personal_tasks::Task};

impl App {
    pub(super) fn submit_read_state(
        &mut self,
        articles: BTreeSet<i64>,
        unread: bool,
    ) -> Result<()> {
        if let Some(writer) = &mut self.personal_writes {
            let ticket = writer.submit(Task::Read {
                articles: articles.clone(),
                unread,
            })?;
            self.pending_reads
                .insert(ticket, (articles.iter().copied().collect(), unread));
        } else {
            self.store.edit_articles(
                &articles,
                &crate::model::BulkEdit::Unread(unread),
            )?;
        }
        for article in &mut self.library.articles {
            if articles.contains(&article.id) {
                article.unread = unread;
            }
        }
        Ok(())
    }

    pub(super) fn complete_read_state(
        &mut self,
        ticket: u64,
        failed: bool,
    ) -> bool {
        if self.pending_reads.remove(&ticket).is_none() {
            return false;
        }
        self.library_revision = self.library_revision.wrapping_add(1);
        self.read_reconcile_required |= failed;
        // Counts and flags come from one WAL snapshot, off the input thread.
        // A later manual unread or undo uses this same ordered writer.
        if self.pending_reads.is_empty() {
            let scope = if self.retained_article.is_some()
                && !self.read_reconcile_required
            {
                super::catalog_worker::Scope::Counts
            } else {
                super::catalog_worker::Scope::Library
            };
            self.request_catalog(scope);
        }
        true
    }

    pub(super) fn overlay_pending_reads(&mut self) {
        for (ids, unread) in self.pending_reads.values() {
            for article in &mut self.library.articles {
                if ids.contains(&article.id) {
                    article.unread = *unread;
                }
            }
        }
    }
}
