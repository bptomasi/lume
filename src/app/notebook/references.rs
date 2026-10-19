//! Reference navigation records the note query before following a stable ID.

use std::sync::Arc;

use anyhow::{Context, Result};

use super::super::{Action, App, Choice, ChoiceAction};

/// Captured note action, independent of later article or result selection.
#[derive(Clone, Debug)]
pub enum NoteAction {
    /// Follow an existing note while retaining the previous search.
    Open(i64),
    /// Resume one durable draft without deleting its checkpoint.
    Recover(Arc<crate::model::NoteDraft>),
    /// Inspect old and current text before restoration.
    PreviewVersion {
        /// Captured current note and ownership.
        current: Arc<crate::model::AnnotationSnapshot>,
        /// Captured historical content.
        version: Arc<crate::model::NoteVersion>,
    },
    /// Restore only the version explicitly previewed by the user.
    RestoreVersion {
        /// Expected current note.
        current: Arc<crate::model::AnnotationSnapshot>,
        /// Owned historical version.
        version: i64,
    },
    /// Confirm a replacement source range after its preview.
    Reanchor {
        /// Expected current note and article.
        current: Arc<crate::model::AnnotationSnapshot>,
        /// Selected source evidence.
        passage: crate::model::Passage,
    },
    /// Insert a stable Markdown note reference into a suspended editor.
    InsertReference(i64),
    /// Restore one previous explicit search.
    Query(String),
    /// Assign a type chosen from the note kind picker.
    Kind {
        /// Owning article.
        article: i64,
        /// Captured annotation.
        note: i64,
        /// Explicit kind.
        kind: crate::model::NoteKind,
    },
}

impl App {
    pub(in crate::app) fn show_note_references(&mut self) -> Result<()> {
        let note = self.note_target()?;
        self.inspect_notes(super::inspection::Inspection::References(
            note.comment.id,
        ))
    }

    pub(in crate::app) fn show_note_kinds(&mut self) -> Result<()> {
        let note = self.note_target()?;
        let choices = crate::model::NoteKind::ALL.into_iter().map(|kind| Choice {
            title: kind.name().into(),
            detail: "Assign meaning to this note without changing its source or tags".into(),
            action: ChoiceAction::Note(NoteAction::Kind {
                article: note.article,
                note: note.comment.id,
                kind,
            }),
        }).collect();
        self.inspect(choices)
    }

    pub(in crate::app) fn apply_note_action(
        &mut self,
        action: NoteAction,
    ) -> Result<Action> {
        if self.apply_note_editor_action(&action)? {
            return Ok(Action::None);
        }
        match action {
            NoteAction::Open(id) => {
                self.follow_note_reference(id)?;
            }
            NoteAction::Kind {
                article,
                note,
                kind,
            } => {
                let change = crate::model::NoteChange::Kind(kind);
                if let Some(writer) = &mut self.note_tasks {
                    writer.submit(super::super::note_tasks::Task::Edit(
                        crate::app::NoteEdit::Metadata {
                            article,
                            note,
                            change,
                        },
                    ))?;
                } else {
                    self.store.change_note_metadata(article, note, change)?;
                    self.refresh_notes(article)?;
                }
            }
            NoteAction::Query(query) => self.set_notebook_query(&query)?,
            _ => {}
        }
        Ok(Action::None)
    }
    fn follow_note_reference(&mut self, id: i64) -> Result<()> {
        if self.notebook.is_none() {
            self.open_notebook("", None)?;
        }
        let notebook = self.notebook.as_mut().context("Open Notebook first")?;
        if notebook.history.len() == 128 {
            notebook.history.remove(0);
        }
        notebook.history.push((
            notebook.query.clone(),
            notebook.offset,
            notebook.selected,
        ));
        self.set_notebook_query(&format!("id:{id}"))?;
        Ok(())
    }
}
