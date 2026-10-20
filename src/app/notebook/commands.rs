//! Explicit notebook commands share targets with keyboard and mouse actions.

use anyhow::{Context, Result, ensure};

use crate::model::{AnnotationSnapshot, NoteChange, NoteKind};

use super::super::{
    Action, App, Focus, InputKind,
    personal_tasks::{QueryEdit, Task},
};

impl App {
    pub(in crate::app) fn guard_notebook_article_command(
        &self,
        verb: &str,
    ) -> Result<()> {
        if self.notebook.is_none() {
            return Ok(());
        }
        // The article page is hidden in Notebook. Explicit source navigation
        // must precede mutations of that page or its multi-article selection.
        let article_action = matches!(
            verb,
            "favorite"
                | "mark-read"
                | "mark-unread"
                | "save"
                | "organize"
                | "delete-article"
                | "restore-article"
                | "bulk"
                | "tags"
                | "select-article"
                | "select-page"
                | "clear-selection"
                | "rename"
                | "delete-folder"
                | "unfeed"
                | "delete-smart"
                | "pin"
                | "move-up"
                | "move-down"
                | "collapse"
                | "fetch"
                | "content"
                | "extract-preview"
                | "extract-rule"
                | "extract-auto"
                | "links"
                | "toc"
                | "tables"
                | "copy-code"
        );
        ensure!(
            !article_action,
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
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        if self.notebook_inspection_command(verb, argument)? {
            return Ok(Some(Action::None));
        }
        match verb {
            "notes" => self.open_note_query(argument)?,
            "note-search" => {
                if argument.is_empty() {
                    self.begin_global_note_search()?;
                } else {
                    self.open_note_query(argument)?;
                }
            }
            "note-collection" => {
                ensure!(
                    self.notebook.is_some(),
                    "Open Notebook and choose a query first"
                );
                if argument.is_empty() {
                    self.begin_input(InputKind::NoteCollection, String::new());
                } else {
                    self.save_current_note_collection(argument)?;
                }
            }
            "note-type" | "note-status" | "note-pin" => {
                self.change_note_metadata(verb, argument)?
            }
            "note-tags" => self.open_note_tags(argument)?,
            "highlight-tags" => self.open_note_tags(argument)?,
            "note-backlinks" => self.show_note_references()?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
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
            if let Some(notebook) = &mut self.notebook {
                notebook.recent = self.store.note_search_history()?;
            }
        }
        Ok(())
    }

    fn notebook_inspection_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<bool> {
        use super::inspection::Inspection;
        match verb {
            "drafts" => self.inspect_notes(Inspection::Drafts)?,
            "export-note-files" => {
                self.export_note_files(argument)?;
            }
            "note-history" => {
                let note = std::sync::Arc::new(self.note_target()?);
                self.inspect_notes(Inspection::Versions(note))?;
            }
            "note-reanchor" => self.preview_note_anchor()?,
            "note-synthesis" => self.synthesize_notes()?,
            "note-recent" => {
                self.show_recent_note_searches()?;
            }
            "note-collection-delete" => {
                if let Some(writer) = &mut self.personal_writes {
                    writer.submit(Task::Query(QueryEdit::Delete(
                        argument.into(),
                    )))?;
                } else {
                    self.store.delete_note_collection(argument)?;
                    self.invalidate_notebook();
                }
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn show_recent_note_searches(&mut self) -> Result<()> {
        let notebook = self.notebook.as_ref().context("Open Notebook first")?;
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
        if let Some(notebook) = &self.notebook {
            return notebook
                .current_context()
                .map(|context| context.note.clone())
                .context("Wait for the selected note's context");
        }
        self.guard_stale_command("note")?;
        let article = self.current().context("Select an article")?.id;
        let id = self
            .cursor_note()
            .or_else(|| {
                self.comments.get(self.comment_selected).map(|note| note.id)
            })
            .context("Select a note first")?;
        self.store.annotation(article, id)
    }

    pub(in crate::app) fn change_note_metadata(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<()> {
        let note = self.note_target()?;
        let change = match verb {
            "note-type" => NoteChange::Kind(NoteKind::named(argument)?),
            "note-status" => {
                ensure!(
                    matches!(argument, "open" | "resolved"),
                    "Use open or resolved"
                );
                NoteChange::Resolved(argument == "resolved")
            }
            "note-status-toggle" => NoteChange::ToggleResolved,
            _ => NoteChange::TogglePin,
        };
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
            .notebook
            .as_ref()
            .context("Open Notebook and choose a query first")?;
        crate::store::validate_collection_name(name)?;
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
            self.notebook.is_none() && self.focus == Focus::Reader,
            "Focus Reader before creating an article note"
        );
        ensure!(
            self.current().is_some() && !self.article_preparing(),
            "Wait for a readable article before creating a note"
        );
        self.edit_comment(false)
    }
}
