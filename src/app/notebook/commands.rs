//! Explicit notebook commands share targets with keyboard and mouse actions.

use crate::{app::commands::routing, command::Command};

use anyhow::{Context, Result, ensure};

use crate::model::{AnnotationSnapshot, NoteChange, NoteKind};

use super::super::{
    Action, App, Focus, InputKind,
    personal_tasks::{QueryEdit, Task},
};

impl App {
    pub(in crate::app) fn guard_notebook_article_command(
        &self,
        command: Command,
    ) -> Result<()> {
        if self.notebooks.active.is_none() {
            return Ok(());
        }
        // The article page is hidden in Notebook. Explicit source navigation
        // must precede mutations of that page or its multi-article selection.
        ensure!(
            !command.requires_visible_article(),
            "Open the source with Enter before using article actions; p pins a note"
        );
        Ok(())
    }

    pub(in crate::app) fn notebook_input(
        &mut self,
        input: &super::super::Input,
    ) -> Result<bool> {
        match input.kind {
            InputKind::NoteTags { article, note } => {
                self.save_note_tags(article, note, &input.text)?;
            }
            InputKind::NoteCollection => {
                self.save_current_note_collection(&input.text)?;
            }
            InputKind::NoteSearch => {
                self.remember_note_query(&input.text)?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    pub(in crate::app) fn notebook_command(
        &mut self,
        command: routing::NotebookCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::NotebookCommand as C;

        match command {
            C::Notes => self.open_note_query(argument)?,
            C::NoteSearch => {
                if argument.is_empty() {
                    self.begin_global_note_search()?;
                } else {
                    self.open_note_query(argument)?;
                }
            }
            C::NoteCollection => {
                ensure!(
                    self.notebooks.active.is_some(),
                    "Open Notebook and choose a query first"
                );
                if argument.is_empty() {
                    self.begin_input(InputKind::NoteCollection, String::new());
                } else {
                    self.save_current_note_collection(argument)?;
                }
            }
            C::NoteType => self.change_note_kind(argument)?,
            C::NoteStatus => self.change_note_status(argument)?,
            C::NotePin => self.change_note_metadata(NoteChange::TogglePin)?,
            C::NoteTags | C::HighlightTags => self.open_note_tags(argument)?,
            C::NoteBacklinks => self.show_note_references()?,
        }
        Ok(Action::None)
    }

    fn open_note_query(&mut self, query: &str) -> Result<()> {
        self.open_notebook(query, None)?;
        self.remember_note_query(query)
    }

    fn remember_note_query(&mut self, query: &str) -> Result<()> {
        crate::search::notes::validate(query)?;
        if query.trim().is_empty() {
            return Ok(());
        }
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Query(QueryEdit::Remember(query.into())))?;
        } else {
            self.store.remember_note_search(query)?;
            if let Some(notebook) = &mut self.notebooks.active {
                notebook.install_recent(self.store.note_search_history()?);
            }
        }
        Ok(())
    }

    pub(in crate::app) fn notebook_inspection_command(
        &mut self,
        command: routing::NotebookInspectionCommand,
        argument: &str,
    ) -> Result<Action> {
        use super::inspection::Inspection;
        match command {
            routing::NotebookInspectionCommand::Drafts => {
                self.inspect_notes(Inspection::Drafts)?
            }
            routing::NotebookInspectionCommand::ExportNoteFiles => {
                self.export_note_files(argument)?;
            }
            routing::NotebookInspectionCommand::NoteHistory => {
                let note = std::sync::Arc::new(self.note_target()?);
                self.inspect_notes(Inspection::Versions(note))?;
            }
            routing::NotebookInspectionCommand::NoteReanchor => {
                self.preview_note_anchor()?
            }
            routing::NotebookInspectionCommand::NoteSynthesis => {
                self.synthesize_notes()?
            }
            routing::NotebookInspectionCommand::NoteRecent => {
                self.show_recent_note_searches()?;
            }
            routing::NotebookInspectionCommand::NoteCollectionDelete => {
                if let Some(writer) = &mut self.personal_writes {
                    writer.submit(Task::Query(QueryEdit::Delete(
                        argument.into(),
                    )))?;
                } else {
                    self.store.delete_note_collection(argument)?;
                    self.invalidate_notebook();
                }
            }
        }
        Ok(Action::None)
    }

    fn show_recent_note_searches(&mut self) -> Result<()> {
        let notebook = self
            .notebooks
            .active
            .as_ref()
            .context("Open Notebook first")?;
        let choices = notebook
            .recent
            .iter()
            .map(|query| super::super::Choice {
                title: query.clone(),
                detail: "Restore this previous note query".into(),
                action: super::super::ChoiceAction::Note(
                    super::NoteAction::Query(query.clone()),
                ),
            })
            .collect();
        self.inspect(choices)
    }

    fn export_note_files(&mut self, argument: &str) -> Result<()> {
        ensure!(!argument.is_empty(), "Use :export-note-files DIRECTORY");
        let path = std::path::PathBuf::from(argument);
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Export(path))?;
        } else {
            let report = crate::exchange::notes::notebook(&self.store, &path)?;
            self.status = format!(
                "Note files · {} written · {} conflicts preserved",
                report.written,
                report.conflicts.len()
            );
        }
        Ok(())
    }

    pub(in crate::app) fn note_target(&self) -> Result<AnnotationSnapshot> {
        if let Some(notebook) = &self.notebooks.active {
            return notebook
                .current_context()
                .map(|context| context.note.clone())
                .context("Wait for the selected note's context");
        }
        self.guard_stale_command(crate::command::Command::Note)?;
        let article = self.current().context("Select an article")?.id;
        let id = self
            .cursor_note()
            .or_else(|| {
                self.reader
                    .view()
                    .comments
                    .get(self.reader.view().comment_selected)
                    .map(|note| note.id)
            })
            .context("Select a note first")?;
        self.store.annotation(article, id)
    }

    pub(in crate::app) fn change_note_metadata(
        &mut self,
        change: NoteChange,
    ) -> Result<()> {
        let note = self.note_target()?;
        self.change_captured_note_metadata(note, change)
    }

    fn change_note_kind(&mut self, argument: &str) -> Result<()> {
        let note = self.note_target()?;
        let change = NoteChange::Kind(NoteKind::named(argument)?);
        self.change_captured_note_metadata(note, change)
    }

    fn change_note_status(&mut self, argument: &str) -> Result<()> {
        let note = self.note_target()?;
        ensure!(
            matches!(argument, "open" | "resolved"),
            "Use open or resolved"
        );
        self.change_captured_note_metadata(
            note,
            NoteChange::Resolved(argument == "resolved"),
        )
    }

    fn change_captured_note_metadata(
        &mut self,
        note: AnnotationSnapshot,
        change: NoteChange,
    ) -> Result<()> {
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Edit(crate::app::NoteEdit::Metadata {
                article: note.article,
                note: note.comment.id,
                change,
            }))?;
            self.status = "Updating note metadata…".into();
            return Ok(());
        }
        self.store.change_note_metadata(
            note.article,
            note.comment.id,
            change,
        )?;
        self.refresh_notes(note.article)
    }

    pub(in crate::app) fn open_note_tags(
        &mut self,
        argument: &str,
    ) -> Result<()> {
        let note = self.note_target()?;
        if !argument.is_empty() {
            return self.save_note_tags(
                note.article,
                note.comment.id,
                argument,
            );
        }
        self.begin_input(
            InputKind::NoteTags {
                article: note.article,
                note: note.comment.id,
            },
            note.tags.join(" "),
        );
        Ok(())
    }

    pub(in crate::app) fn save_note_tags(
        &mut self,
        article: i64,
        note: i64,
        text: &str,
    ) -> Result<()> {
        let tags = if text.trim() == "clear" {
            Vec::new()
        } else {
            text.split_whitespace().map(str::to_owned).collect()
        };
        let tags = crate::store::validate_tags(&tags)?;
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Edit(crate::app::NoteEdit::Tags {
                article,
                note,
                tags,
            }))?;
            self.status = "Updating note tags…".into();
            return Ok(());
        }
        self.store.annotation(article, note)?;
        self.store
            .set_tags(crate::model::TagTarget::Comment(note), &tags)?;
        self.refresh_notes(article)
    }

    pub(in crate::app) fn save_current_note_collection(
        &mut self,
        name: &str,
    ) -> Result<()> {
        let notebook = self
            .notebooks
            .active
            .as_ref()
            .context("Open Notebook and choose a query first")?;
        crate::validation::validate_collection_name(name)?;
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(Task::Query(QueryEdit::Save {
                name: name.into(),
                query: notebook.query.clone(),
            }))?;
        } else {
            self.store.save_note_collection(name, &notebook.query)?;
            self.invalidate_notebook();
        }
        self.status = "Note collection saved".into();
        Ok(())
    }

    pub(in crate::app) fn begin_reader_note(&mut self) -> Result<()> {
        ensure!(
            self.notebooks.active.is_none() && self.focus == Focus::Reader,
            "Focus Reader before creating an article note"
        );
        ensure!(
            self.current().is_some() && !self.article_preparing(),
            "Wait for a readable article before creating a note"
        );
        self.edit_comment(false)
    }
}
