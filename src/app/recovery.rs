//! Debounced recovery snapshots never replace admitted user mutations.
//!
//! A quiet editor checkpoints after 250 ms; continuous typing checkpoints at
//! least once per second while the writer is available. Explicit exit flushes
//! admitted writes and checkpoints any remaining dirty buffer before returning.

use std::time::{Duration, Instant};

use anyhow::Result;

use crate::model::NoteDraft;

use super::{App, Input, Mode, personal_tasks::Task};

#[derive(Default)]
pub(super) struct Recovery {
    snapshot: Option<NoteDraft>,
    due: Option<Instant>,
    since: Option<Instant>,
}

impl Recovery {
    fn observe(&mut self, input: Option<&Input>, now: Instant) {
        let draft = input.filter(|input| input.modified()).and_then(snapshot);
        if self.snapshot == draft {
            return;
        }
        self.snapshot = draft;
        if self.snapshot.is_none() {
            self.due = None;
            self.since = None;
            return;
        }
        let since = *self.since.get_or_insert(now);
        self.due = Some(
            (now + Duration::from_millis(250))
                .min(since + Duration::from_secs(1)),
        );
    }

    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
}

pub(super) fn snapshot(input: &Input) -> Option<NoteDraft> {
    Some(NoteDraft {
        target: input.draft_target()?,
        body: input.text.clone(),
        baseline: input.saved_text.clone(),
        initial_kind: input.initial_kind,
    })
}

impl App {
    pub(super) fn ensure_no_recovery(
        &self,
        target: &crate::model::DraftTarget,
    ) -> Result<()> {
        anyhow::ensure!(
            !self.store.has_note_draft(target)?,
            "An interrupted buffer exists for this note · :drafts recovers it before editing"
        );
        Ok(())
    }

    pub(super) fn observe_draft(&mut self) {
        let input = match &self.mode {
            Mode::Input(input) => Some(input.as_ref()),
            _ => self.suspended_note.as_deref(),
        };
        self.recovery.observe(input, Instant::now());
    }

    pub(super) fn tick_draft_recovery(&mut self) -> Result<()> {
        if self.pending_note_write.is_some()
            || self.recovery.due.is_none_or(|due| due > Instant::now())
        {
            return Ok(());
        }
        let Some(draft) = self.recovery.snapshot.clone() else {
            return Ok(());
        };
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Checkpoint(draft))?;
        } else {
            self.store.save_note_draft(&draft)?;
        }
        self.recovery.due = None;
        self.recovery.since = None;
        Ok(())
    }

    pub(super) fn discard_draft_checkpoint(
        &mut self,
        input: &Input,
    ) -> Result<()> {
        if let Some(target) = input.draft_target() {
            if let Some(writer) = &mut self.personal_writes {
                writer.submit(Task::Discard(target))?;
            } else {
                self.store.discard_note_draft(&target)?;
            }
        }
        self.recovery.reset();
        Ok(())
    }

    /// Checkpoint the current dirty buffer at an explicit durability boundary.
    ///
    /// # Errors
    /// Returns pending transaction failures or recovery storage errors.
    pub fn checkpoint_draft(&mut self) -> Result<()> {
        self.flush_note_tasks()?;
        let input = match &self.mode {
            Mode::Input(input) => Some(input.as_ref()),
            _ => self.suspended_note.as_deref(),
        };
        if let Some(input) = input.filter(|input| input.modified())
            && let Some(draft) = snapshot(input)
        {
            self.store.save_note_draft(&draft)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::InputKind;

    fn dirty_note() -> Input {
        let mut input = Input::new(
            InputKind::Comment {
                article: 1,
                comment: None,
            },
            "One observation".into(),
            String::new(),
        );
        input.saved_text.clear();
        input
    }

    #[test]
    fn quiet_typing_debounces_but_continuous_edits_keep_a_one_second_deadline()
    {
        let mut recovery = Recovery::default();
        let mut input = dirty_note();
        let start = Instant::now();
        recovery.observe(Some(&input), start);
        assert_eq!(recovery.due, Some(start + Duration::from_millis(250)));
        for elapsed in [200, 400, 600, 800, 950] {
            input.text.push('.');
            recovery
                .observe(Some(&input), start + Duration::from_millis(elapsed));
        }
        assert_eq!(recovery.due, Some(start + Duration::from_secs(1)));
    }

    #[test]
    fn unchanged_observations_do_not_delay_the_checkpoint_and_clean_text_cancels_it()
     {
        let mut recovery = Recovery::default();
        let mut input = dirty_note();
        let start = Instant::now();
        recovery.observe(Some(&input), start);
        recovery.observe(Some(&input), start + Duration::from_millis(200));
        assert_eq!(recovery.due, Some(start + Duration::from_millis(250)));
        input.saved_text.clone_from(&input.text);
        recovery.observe(Some(&input), start + Duration::from_millis(220));
        assert!(recovery.due.is_none());
        assert!(recovery.snapshot.is_none());
    }
}
