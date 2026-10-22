//! UI completions update only the captured note and preserve newer draft text.

use anyhow::{Context, Result, ensure};

use super::{
    Action, App, InputKind, Mode,
    draft_commands::{Destination, ExCommand},
    personal_tasks::{Completion, Outcome, PendingWrite, Task},
};

impl App {
    pub(super) fn defer_draft_command(
        &mut self,
        command: ExCommand,
    ) -> Result<bool> {
        if self.personal_writes.is_none() {
            return Ok(false);
        }
        let ExCommand::Write(destination) = command else {
            ensure!(
                self.pending_note_write.is_none(),
                "Wait for the pending note write before closing"
            );
            return Ok(false);
        };
        let Mode::Input(input) = &self.mode else {
            return Ok(false);
        };
        if !input.modified() && super::draft_save::persisted(&input.kind) {
            return Ok(false);
        }
        self.request_draft_write(destination)?;
        Ok(true)
    }

    pub(super) fn request_draft_write(
        &mut self,
        destination: Destination,
    ) -> Result<()> {
        ensure!(
            self.pending_note_write.is_none(),
            "A write is pending; editing remains available"
        );
        let Mode::Input(input) = &self.mode else {
            anyhow::bail!("Open a note editor first");
        };
        let draft = crate::model::NoteDraft {
            target: input
                .draft_target()
                .context("Open a note/card editor first")?,
            body: input.text.clone(),
            baseline: input.saved_text.clone(),
            initial_kind: input.initial_kind,
        };
        let writer = self
            .personal_writes
            .as_mut()
            .context("Note writer is unavailable")?;
        let ticket = writer.submit(Task::Write(draft))?;
        if let Mode::Input(input) = &mut self.mode {
            input.edit_mode = super::EditMode::Normal;
        }
        self.pending_note_write = Some(PendingWrite {
            ticket,
            destination,
        });
        self.status = "Saving note… · editing remains available".into();
        Ok(())
    }

    /// Apply ready note writes without waiting for SQLite or another thread.
    ///
    /// # Errors
    /// Returns captured write/refresh failures while retaining the editor.
    pub fn poll_note_tasks(&mut self) -> Result<Option<Action>> {
        self.tick_draft_recovery()?;
        let mut changed = false;
        while let Some(completion) = self
            .personal_writes
            .as_ref()
            .and_then(|writer| writer.poll())
        {
            changed = true;
            let action = self.apply_note_completion(completion)?;
            if action == Action::Quit {
                return Ok(Some(action));
            }
        }
        Ok(changed.then_some(Action::None))
    }

    /// Poll all annotation work without waiting for storage or preparation.
    ///
    /// # Errors
    /// Returns captured transaction/read failures while retaining user drafts.
    pub fn poll_note_workspace(&mut self) -> Result<Option<Action>> {
        let action = self.poll_note_tasks()?;
        if action == Some(Action::Quit) {
            return Ok(action);
        }
        let mut changed = action.is_some();
        changed |= self.poll_note_catalog()?;
        changed |= self.poll_note_inspection()?;
        changed |= self.poll_notebook()?;
        Ok(changed.then_some(Action::None))
    }

    fn apply_note_completion(
        &mut self,
        completion: Completion,
    ) -> Result<Action> {
        if let Some(started) = completion.started {
            self.performance
                .record(super::Operation::Save, started.elapsed());
        }
        self.pending_note_deletes.remove(&completion.ticket);
        let pending = self.take_completed_write(completion.ticket);
        if self.complete_article_edit(
            completion.ticket,
            completion.result.is_err(),
        ) {
            completion.result?;
            return Ok(Action::None);
        }
        match completion.result? {
            Outcome::SavedPage { article, visit } => {
                self.finish_web_save(article, visit)?
            }
            Outcome::Written {
                draft,
                target,
                comments,
            } => {
                return self
                    .finish_written_note(draft, target, comments, pending);
            }
            Outcome::Changed { effect, comments } => {
                self.install_personal_effect(effect, comments?)?;
            }
            Outcome::Articles | Outcome::Checkpoint => {}
            Outcome::Collections => self.invalidate_notebook(),
            Outcome::SearchHistory(history) => {
                if let Some(notebook) = &mut self.notebook {
                    notebook.install_recent(history);
                }
            }
            Outcome::Exported(report) => {
                self.show_note_export(report);
            }
        }
        Ok(Action::None)
    }

    fn finish_written_note(
        &mut self,
        draft: crate::model::NoteDraft,
        target: crate::model::DraftTarget,
        comments: Result<Vec<crate::model::Comment>>,
        pending: Option<PendingWrite>,
    ) -> Result<Action> {
        self.install_saved_identity(&draft, target.clone())?;
        self.install_note_changes(target.article(), comments?);
        self.select_written_note(&target);
        let action = if let Some(pending) = pending {
            self.finish_note_write(pending.destination)?
        } else {
            self.status = "Annotation saved · u undo".into();
            Action::None
        };
        self.observe_draft();
        Ok(action)
    }

    fn select_written_note(&mut self, target: &crate::model::DraftTarget) {
        if !self
            .current()
            .is_some_and(|article| article.id == target.article())
        {
            return;
        }
        let note = match target {
            crate::model::DraftTarget::Note { note, .. } => *note,
            crate::model::DraftTarget::Card { note, .. } => Some(*note),
            _ => None,
        };
        if let Some(index) =
            self.comments.iter().position(|item| Some(item.id) == note)
        {
            self.comment_selected = index;
        }
    }

    fn take_completed_write(&mut self, ticket: u64) -> Option<PendingWrite> {
        if self
            .pending_note_write
            .as_ref()
            .is_some_and(|pending| pending.ticket == ticket)
        {
            self.pending_note_write.take()
        } else {
            None
        }
    }

    fn show_note_export(
        &mut self,
        report: crate::exchange::incremental::ExportReport,
    ) {
        self.status = format!(
            "Note files · {} written · {} unchanged · {} conflicts preserved",
            report.written,
            report.unchanged,
            report.conflicts.len()
        );
        if !report.conflicts.is_empty() {
            let text = report
                .conflicts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join("\n");
            self.show_document(
                "Export conflicts · local files preserved",
                &text,
                None,
            );
        }
    }

    fn install_saved_identity(
        &mut self,
        draft: &crate::model::NoteDraft,
        target: crate::model::DraftTarget,
    ) -> Result<()> {
        let Mode::Input(input) = &mut self.mode else {
            return Ok(());
        };
        ensure!(
            input
                .draft_target()
                .is_some_and(|target| target.key() == draft.target.key()),
            "Saved note belongs to another editor; its text remains durable"
        );
        input.kind = InputKind::from_draft_target(target);
        input.saved_text.clone_from(&draft.body);
        input.initial_kind = None;
        Ok(())
    }

    fn finish_note_write(
        &mut self,
        destination: Destination,
    ) -> Result<Action> {
        let Mode::Input(input) = &mut self.mode else {
            self.status = "Note saved".into();
            return Ok(Action::None);
        };
        if input.modified() {
            self.status =
                "Earlier revision saved · newer draft changes remain unsaved"
                    .into();
            return Ok(Action::None);
        }
        input.edit_mode = super::EditMode::Normal;
        if !matches!(destination, Destination::Editor) {
            self.mode = Mode::Normal;
        }
        self.status = "Note saved · :w keeps editing · :wq closes".into();
        if matches!(destination, Destination::Quit) {
            self.save_session()?;
            self.flush_session()?;
            return Ok(Action::Quit);
        }
        Ok(Action::None)
    }

    fn install_personal_effect(
        &mut self,
        effect: crate::model::UndoEffect,
        comments: Vec<crate::model::Comment>,
    ) -> Result<()> {
        match effect {
            crate::model::UndoEffect::Notes(article) => {
                self.install_note_changes(article, comments)
            }
            crate::model::UndoEffect::Library => {
                self.reload()?;
                self.status = "Personal edit saved · u undo".into();
                return Ok(());
            }
            crate::model::UndoEffect::Unchanged => {
                self.status = "No personal edits to undo".into();
                return Ok(());
            }
        }
        self.status = "Note updated · u undo".into();
        Ok(())
    }

    /// Whether personal transactions still need UI acknowledgement.
    pub fn notes_pending(&self) -> bool {
        self.pending_note_write.is_some()
            || self
                .personal_writes
                .as_ref()
                .is_some_and(|writer| writer.pending())
    }

    pub(super) fn flush_note_tasks(&mut self) -> Result<()> {
        let deadline =
            std::time::Instant::now() + std::time::Duration::from_secs(5);
        let mut failure = None;
        while self.notes_pending() {
            if let Err(error) = self.poll_note_tasks() {
                failure.get_or_insert(error);
            }
            ensure!(
                std::time::Instant::now() < deadline,
                "Personal writes did not finish before shutdown"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        if let Err(error) = self.poll_note_tasks() {
            failure.get_or_insert(error);
        }
        if let Some(error) = failure {
            return Err(error);
        }
        Ok(())
    }
}

impl InputKind {
    pub(super) fn from_draft_target(target: crate::model::DraftTarget) -> Self {
        use crate::model::DraftTarget;
        match target {
            DraftTarget::Note { article, note } => Self::Comment {
                article,
                comment: note,
            },
            DraftTarget::Passage { article, passage } => {
                Self::Highlight { article, passage }
            }
            DraftTarget::Card {
                article,
                note,
                card,
            } => Self::StudyCard {
                article,
                comment: note,
                card,
            },
        }
    }
}
