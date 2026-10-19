//! Historical and recovery inspections prepare bounded choices off input.

use std::sync::Arc;

use anyhow::{Context, Result};

use crate::{latest::Latest, model::AnnotationSnapshot, store::Store};

use super::super::{App, Choice, ChoiceAction};
use super::NoteAction;

pub(in crate::app) enum Inspection {
    Drafts,
    ReviewNotes(String),
    ReviewCards(String),
    Versions(Arc<AnnotationSnapshot>),
    References(i64),
    Link { query: String, exclude: Option<i64> },
    Synthesis(Vec<i64>),
}

pub(in crate::app) enum Prepared {
    Choices(Vec<Choice>),
    Synthesis { article: i64, text: String },
}

pub(in crate::app) struct Inspector {
    worker: Latest<Inspection, Prepared>,
}

impl App {
    pub(in crate::app) fn inspect_notes(
        &mut self,
        request: Inspection,
    ) -> Result<()> {
        if self.note_inspector.is_none() {
            let store = Store::open(&self.store.database_path()?)?;
            self.note_inspector = Some(Inspector {
                worker: Latest::start(
                    "lume-note-inspection",
                    move |request| prepare(&store, request),
                )?,
            });
        }
        self.note_inspector
            .as_mut()
            .context("Note inspection unavailable")?
            .worker
            .submit(request);
        self.status = "Preparing note inspection… · Esc cancels".into();
        Ok(())
    }

    /// Install a ready inspection without replacing an active draft.
    ///
    /// # Errors
    /// Returns inspection failures; a suspended editor is recovered intact.
    pub fn poll_note_inspection(&mut self) -> Result<bool> {
        let Some(result) = self
            .note_inspector
            .as_mut()
            .and_then(|inspector| inspector.worker.poll())
        else {
            return Ok(false);
        };
        match result {
            Ok(Prepared::Choices(choices)) => self.inspect(choices)?,
            Ok(Prepared::Synthesis { article, text }) => {
                self.close_notebook();
                self.jump_article(article)?;
                self.focus = super::super::Focus::Reader;
                self.begin_reader_note()?;
                self.replace_draft(text)?;
                if let super::super::Mode::Input(input) = &mut self.mode {
                    input.initial_kind =
                        Some(crate::model::NoteKind::Synthesis);
                }
            }
            Err(error) => {
                self.resume_note_editor();
                return Err(error);
            }
        }
        Ok(true)
    }

    pub(in crate::app) fn cancel_note_inspection(&mut self) {
        if let Some(inspector) = &mut self.note_inspector {
            inspector.worker.cancel();
        }
        self.resume_note_editor();
    }

    /// Whether a replaceable note inspection remains in flight.
    pub fn note_inspection_pending(&self) -> bool {
        self.note_inspector
            .as_ref()
            .is_some_and(|inspector| inspector.worker.pending())
    }

    pub(in crate::app) fn synthesize_notes(&mut self) -> Result<()> {
        let notes = self
            .notebook
            .as_ref()
            .context("Open Notebook first")?
            .marked
            .iter()
            .copied()
            .collect::<Vec<_>>();
        anyhow::ensure!(!notes.is_empty(), "Use v to select notes first");
        self.inspect_notes(Inspection::Synthesis(notes))
    }
}

fn prepare(store: &Store, request: Inspection) -> Result<Prepared> {
    let choices = match request {
        Inspection::Drafts => drafts(store)?,
        Inspection::ReviewNotes(query) => {
            super::review::note_choices(store, &query)?
        }
        Inspection::ReviewCards(query) => {
            super::review::card_choices(store, &query)?
        }
        Inspection::Versions(note) => versions(store, note)?,
        Inspection::References(id) => references(store, id)?,
        Inspection::Link { query, exclude } => {
            link_choices(store, &query, exclude)?
        }
        Inspection::Synthesis(ids) => return synthesis(store, &ids),
    };
    Ok(Prepared::Choices(choices))
}

fn drafts(store: &Store) -> Result<Vec<Choice>> {
    Ok(store.note_drafts()?.into_iter().map(|draft| {
        let draft = Arc::new(draft);
        Choice {
            title: format!("Recover {}", draft.target.key()),
            detail: format!("{}\n\nEnter resumes this buffer; the checkpoint remains until :w or :q!", draft.body),
            action: ChoiceAction::Note(NoteAction::Recover(draft)),
        }
    }).collect())
}

fn versions(
    store: &Store,
    note: Arc<AnnotationSnapshot>,
) -> Result<Vec<Choice>> {
    Ok(store
        .note_versions(note.article, note.comment.id)?
        .into_iter()
        .map(|version| Choice {
            title: format!(
                "Version #{} · {}",
                version.id,
                chrono::DateTime::from_timestamp(version.saved_at, 0)
                    .map_or_else(String::new, |date| date.to_rfc3339())
            ),
            detail: format!(
                "{}\n\n{}\n\nEnter opens a restoration preview",
                version.comment.quote, version.comment.body
            ),
            action: ChoiceAction::Note(NoteAction::PreviewVersion {
                current: note.clone(),
                version: Arc::new(version),
            }),
        })
        .collect())
}

fn references(store: &Store, id: i64) -> Result<Vec<Choice>> {
    store.note_summary(id)?;
    Ok(store
        .note_backlinks(id)?
        .into_iter()
        .chain(store.note_references(id)?)
        .map(|note| Choice {
            title: format!(
                "#{} {}",
                note.id,
                crate::content::label(&note.body, 90)
            ),
            detail: format!(
                "{}\n\n{}\n\n{}",
                note.title, note.quote, note.body
            ),
            action: ChoiceAction::Note(NoteAction::Open(note.id)),
        })
        .collect())
}

fn link_choices(
    store: &Store,
    text: &str,
    exclude: Option<i64>,
) -> Result<Vec<Choice>> {
    Ok(store
        .search_notes(text, None, 0, 100)?
        .notes
        .into_iter()
        .filter(|note| Some(note.id) != exclude)
        .map(|note| Choice {
            title: format!(
                "Note-{} · {}",
                note.id,
                crate::content::label(&note.body, 90)
            ),
            detail: format!("{}\n\n{}", note.title, note.body),
            action: ChoiceAction::Note(NoteAction::InsertReference(note.id)),
        })
        .collect())
}

fn synthesis(store: &Store, ids: &[i64]) -> Result<Prepared> {
    anyhow::ensure!(!ids.is_empty() && ids.len() <= 20, "Select 1–20 notes");
    let mut text =
        "# Synthesis\n\n## My conclusion\n\n\n## Evidence\n".to_owned();
    let mut article = 0;
    for id in ids {
        let detail = store.note_export(*id)?;
        if article == 0 {
            article = detail.note.article;
        }
        text.push_str(&format!(
            "\n### [[Note-{id}]]\n\nSource: {}\n\n{}\n\n{}\n",
            detail.url, detail.note.comment.quote, detail.note.comment.body
        ));
        anyhow::ensure!(
            text.len() <= 16384,
            "Synthesis exceeds 16 KiB; select fewer notes"
        );
    }
    Ok(Prepared::Synthesis { article, text })
}
