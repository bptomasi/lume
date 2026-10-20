//! Editor inspections preserve the complete buffer, cursor and undo history.

use std::sync::Arc;

use anyhow::{Context, Result, ensure};

use super::super::{
    App, ChoiceAction, EditMode, Input, InputKind, Mode, personal_tasks::Task,
};
use super::{NoteAction, inspection::Inspection};

impl App {
    pub(in crate::app) fn resume_note_editor(&mut self) {
        if let Some(input) = self.suspended_note.take() {
            self.mode = Mode::Input(input);
        }
    }

    pub(in crate::app) fn note_link_completion(
        &mut self,
        query: &str,
    ) -> Result<()> {
        ensure!(
            self.pending_note_write.is_none(),
            "Wait for the pending write first"
        );
        let Mode::Input(input) = &self.mode else {
            anyhow::bail!("Open a note editor first");
        };
        ensure!(
            matches!(
                input.kind,
                InputKind::Comment { .. } | InputKind::Highlight { .. }
            ),
            "References belong to note bodies; open a note editor first"
        );
        let exclude = match input.kind {
            InputKind::Comment { comment, .. } => comment,
            _ => None,
        };
        self.inspect_notes(Inspection::Link {
            query: query.into(),
            exclude,
        })?;
        if let Mode::Input(input) = std::mem::take(&mut self.mode) {
            self.suspended_note = Some(input);
        }
        Ok(())
    }

    pub(super) fn apply_note_editor_action(
        &mut self,
        action: &NoteAction,
    ) -> Result<bool> {
        match action {
            NoteAction::Recover(draft) => self.recover_note(draft)?,
            NoteAction::InsertReference(id) => {
                self.insert_note_reference(*id)?
            }
            NoteAction::PreviewVersion { current, version } => {
                self.show_document(
                    "Restore note version · Enter confirms · Esc cancels",
                    &format!(
                        "CURRENT\n\n{}\n\n{}\n\nVERSION #{}\n\n{}\n\n{}",
                        current.comment.quote,
                        current.comment.body,
                        version.id,
                        version.comment.quote,
                        version.comment.body
                    ),
                    Some(ChoiceAction::Note(NoteAction::RestoreVersion {
                        current: current.clone(),
                        version: version.id,
                    })),
                );
            }
            NoteAction::RestoreVersion { current, version } => {
                self.restore_note(current.clone(), *version)?;
            }
            NoteAction::Reanchor { current, passage } => {
                self.repair_note(current.clone(), passage.clone())?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn recover_note(&mut self, draft: &crate::model::NoteDraft) -> Result<()> {
        ensure!(
            !self.store.is_deleted(draft.target.article())?,
            "Restore the article from Trash first; the draft is retained"
        );
        let mut input = Input::new(
            InputKind::from_draft_target(draft.target.clone()),
            draft.body.clone(),
            String::new(),
        );
        input.saved_text.clone_from(&draft.baseline);
        input.initial_kind = draft.initial_kind;
        self.mode = Mode::Input(Box::new(input));
        self.observe_draft();
        self.status =
            "Recovered draft · :w saves · :q! discards the checkpoint".into();
        Ok(())
    }

    fn insert_note_reference(&mut self, id: i64) -> Result<()> {
        self.resume_note_editor();
        let Mode::Input(input) = &mut self.mode else {
            anyhow::bail!("The note editor is no longer open");
        };
        let reference = format!("[[Note-{id}]]");
        ensure!(
            input.text.len() + reference.len() <= 16384,
            "Draft exceeds 16 KiB"
        );
        let (row, column) = input.editor.cursor();
        let prefix = input.editor.lines()[row]
            .chars()
            .take(column)
            .collect::<String>();
        if prefix.ends_with("[[") {
            input.editor.delete_char();
            input.editor.delete_char();
        }
        input.editor.insert_str(reference);
        input.text = input.editor.lines().join("\n");
        input.edit_mode = EditMode::Insert;
        self.observe_draft();
        Ok(())
    }

    pub(in crate::app) fn preview_note_anchor(&mut self) -> Result<()> {
        ensure!(self.notebook.is_none(), "Open the source article first");
        let (article, passage) = self.captured_passage()?;
        let note = self
            .comments
            .get(self.comment_selected)
            .context("Select a note with [ or ] first")?;
        let current = Arc::new(self.store.annotation(article, note.id)?);
        self.show_document(
            "Repair note anchor · Enter confirms · Esc cancels",
            &format!("OLD QUOTE\n\n{}\n\nNEW QUOTE · {} · byte {}\n\n{}\n\nNOTE\n\n{}",
                current.comment.quote, passage.source.as_str(), passage.anchor,
                passage.quote, current.comment.body),
            Some(ChoiceAction::Note(NoteAction::Reanchor { current, passage })),
        );
        Ok(())
    }

    fn restore_note(
        &mut self,
        current: Arc<crate::model::AnnotationSnapshot>,
        version: i64,
    ) -> Result<()> {
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Edit(
                crate::app::NoteEdit::RestoreVersion { current, version },
            ))?;
        } else {
            self.store.restore_note_version(
                current.article,
                &current.comment,
                version,
            )?;
            self.refresh_notes(current.article)?;
        }
        Ok(())
    }

    fn repair_note(
        &mut self,
        current: Arc<crate::model::AnnotationSnapshot>,
        passage: crate::model::Passage,
    ) -> Result<()> {
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Edit(crate::app::NoteEdit::Reanchor {
                current,
                passage,
            }))?;
        } else {
            self.store.reanchor_note(
                current.article,
                &current.comment,
                &passage,
            )?;
            self.refresh_notes(current.article)?;
        }
        Ok(())
    }
}
