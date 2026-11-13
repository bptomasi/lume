//! Ordered personal writes and their UI acknowledgements share one lifecycle.
//!
//! A completed failure consumed during an incomplete flush is retained until a
//! later completed flush or ordinary poll reports it exactly once.

use super::{
    article_edits::ArticleEdits,
    draft_commands::Destination,
    personal_tasks::{Completion, PendingWrite, PersonalWrites, Task},
    recovery::Recovery,
};
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, path::Path, time::Instant};

#[derive(Default)]
pub(super) struct PersonalState {
    writer: Option<PersonalWrites>,
    pending_draft: Option<PendingWrite>,
    flush_failure: Option<anyhow::Error>,
    deletes: BTreeMap<u64, i64>,
    recovery: Recovery,
    articles: ArticleEdits,
}

impl PersonalState {
    pub(super) fn background(writer: PersonalWrites) -> Self {
        Self {
            writer: Some(writer),
            ..Self::default()
        }
    }

    pub(super) fn enable(&mut self, path: &Path) -> Result<()> {
        ensure!(!self.pending(), "Personal writes are still outstanding");
        let background = Self::background(PersonalWrites::start(path)?);
        self.writer = background.writer;
        Ok(())
    }

    pub(super) fn enabled(&self) -> bool {
        self.writer.is_some()
    }

    pub(super) fn submit(&mut self, task: Task) -> Result<u64> {
        self.writer
            .as_mut()
            .context("Personal writer is unavailable")?
            .submit(task)
    }

    pub(super) fn submit_draft(
        &mut self,
        draft: crate::model::NoteDraft,
        destination: Destination,
    ) -> Result<()> {
        ensure!(
            self.pending_draft.is_none(),
            "A write is pending; editing remains available"
        );
        let ticket = self.submit(Task::Write(draft))?;
        self.pending_draft = Some(PendingWrite {
            ticket,
            destination,
        });
        Ok(())
    }

    pub(super) fn draft_pending(&self) -> bool {
        self.pending_draft.is_some()
    }

    pub(super) fn acknowledge_draft(
        &mut self,
        ticket: u64,
    ) -> Option<PendingWrite> {
        self.deletes.remove(&ticket);
        if self
            .pending_draft
            .as_ref()
            .is_some_and(|pending| pending.ticket == ticket)
        {
            self.pending_draft.take()
        } else {
            None
        }
    }

    pub(super) fn deleting(&self, note: i64) -> bool {
        self.deletes.values().any(|pending| *pending == note)
    }

    pub(super) fn remember_delete(&mut self, ticket: u64, note: i64) {
        self.deletes.insert(ticket, note);
    }

    pub(super) fn pending(&self) -> bool {
        self.draft_pending()
            || self.writer.as_ref().is_some_and(PersonalWrites::pending)
    }

    pub(super) fn poll(&self) -> Option<Completion> {
        self.writer.as_ref()?.poll()
    }

    pub(super) fn wait(&self, deadline: Instant) -> Result<Completion> {
        self.writer
            .as_ref()
            .context("Personal writer is unavailable before acknowledgement")?
            .wait(deadline)
    }

    pub(super) fn take_flush_failure(&mut self) -> Option<anyhow::Error> {
        self.flush_failure.take()
    }

    pub(super) fn retain_flush_failure(
        &mut self,
        failure: Option<anyhow::Error>,
    ) {
        self.flush_failure = failure;
    }

    pub(super) fn set_diagnostics(&mut self, enabled: bool) {
        if let Some(writer) = &mut self.writer {
            writer.set_diagnostics(enabled);
        }
    }

    pub(super) fn recovery(&self) -> &Recovery {
        &self.recovery
    }

    pub(super) fn recovery_input(&mut self) -> &mut Recovery {
        &mut self.recovery
    }

    pub(super) fn articles(&self) -> &ArticleEdits {
        &self.articles
    }

    pub(super) fn article_editing(&mut self) -> &mut ArticleEdits {
        &mut self.articles
    }
}
