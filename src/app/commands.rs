//! Explicit application operations and the small ex command vocabulary.

use anyhow::{Result, bail};

use super::{Action, App, Input, InputKind, View};

impl App {
    pub(super) fn apply_input(&mut self, input: &mut Input) -> Result<Action> {
        if self.notebook_input(input)? {
            return Ok(Action::None);
        }
        match &input.kind {
            InputKind::NoteTags { .. }
            | InputKind::NoteCollection
            | InputKind::NoteSearch => return Ok(Action::None),
            InputKind::Search => {
                return self.commit_article_search().map(|()| Action::None);
            }
            InputKind::LibrarySearch => {
                return self.commit_library_search().map(|()| Action::None);
            }
            InputKind::ReaderSearch => {
                self.search_reader(&input.text)?;
                return Ok(Action::None);
            }
            InputKind::StudyCard { .. }
            | InputKind::Highlight { .. }
            | InputKind::Comment { .. } => {
                return self.write_draft(input).map(|_| Action::None);
            }
            InputKind::Command => return self.command(&input.text),
            InputKind::NewList => {
                self.save_folder(&input.text, None)?;
                self.status =
                    "Folder created · a organizes the selected article".into();
            }
            InputKind::AddToList(article) => {
                self.save_folder(&input.text, Some(*article))?;
                self.status = format!("Saved to {}", input.text.trim());
            }
            InputKind::Discover
            | InputKind::Import
            | InputKind::ExportArticle(_)
            | InputKind::ExportNotebook
            | InputKind::ExportOpml => return self.exchange_input(input),
        }
        self.reload()?;
        Ok(Action::None)
    }

    pub(crate) fn command(&mut self, text: &str) -> Result<Action> {
        if let Some(action) = self.session_command(text)? {
            return Ok(action);
        }
        let (verb, argument) = command_parts(text);
        if let Some(action) = self.web_command(verb, argument)? {
            return Ok(action);
        }
        anyhow::ensure!(
            !self.web.active || matches!(verb, "help" | "performance"),
            "Linked pages are read-only · :save-page adds one to the library"
        );
        if let Some(action) = self.maintenance_command(verb, argument)? {
            return Ok(action);
        }
        self.guard_notebook_article_command(verb)?;
        self.guard_stale_command(verb)?;
        self.dispatch_command(verb, argument)
    }

    fn dispatch_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Action> {
        type Handler = fn(&mut App, &str, &str) -> Result<Option<Action>>;
        let handlers: &[Handler] = &[
            App::recommendation_command,
            App::layout_commands,
            App::panel_command,
            App::reading_command,
            App::organization_command,
            App::server_command,
            App::study_command,
            App::settings_command,
            App::exchange_command,
            App::workspace_command,
            App::folder_command,
        ];
        for handler in handlers {
            if let Some(action) = handler(self, verb, argument)? {
                return Ok(action);
            }
        }
        if let Some(action) = self.article_command(verb, argument)? {
            return Ok(action);
        }
        self.library_command(verb, argument)
    }

    fn reading_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        if verb == "performance" {
            return self.performance_command(argument).map(Some);
        }
        if let Some(action) = self.notebook_command(verb, argument)? {
            return Ok(Some(action));
        }
        if let Some(action) = self.extraction_command(verb, argument)? {
            return Ok(Some(action));
        }
        if let Some(action) = self.reader_tools_command(verb, argument)? {
            return Ok(Some(action));
        }
        if let Some(action) = self.image_command(verb, argument)? {
            return Ok(Some(action));
        }
        self.document_command(verb, argument)
    }

    fn library_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Action> {
        match verb {
            "r" | "refresh" => return Ok(Action::Refresh),
            "feed" => {
                self.store.subscribe(argument)?;
                self.reload()?;
                self.status = "Feed added; collecting in the background".into();
                return Ok(Action::Refresh);
            }
            "rename" | "delete-folder" => self.edit_folder(verb, argument)?,
            "delete-comment" => {
                let note = self.note_target()?;
                self.delete_captured_note(note.article, note.comment.id)?;
                return Ok(Action::None);
            }
            "unfeed" => {
                self.unsubscribe()?;
                return Ok(Action::None);
            }
            "read" => {
                self.set_view(View::Read)?;
                return Ok(Action::None);
            }
            "mark-read" | "mark-unread" => {
                self.set_read_state(verb == "mark-unread")?;
                return Ok(Action::None);
            }
            "help" => {
                self.show_help(argument)?;
                return Ok(Action::None);
            }
            _ => {
                bail!("Unknown command · Tab completes · :help shows the guide")
            }
        }
        self.reload()?;
        self.status = "Library updated".into();
        Ok(Action::None)
    }

    fn save_folder(&mut self, name: &str, article: Option<i64>) -> Result<()> {
        crate::store::validate_collection_name(name)?;
        self.submit_library_edit(
            super::personal_tasks::LibraryEdit::CreateFolder {
                name: name.into(),
                article,
            },
        )
    }

    fn edit_folder(&mut self, verb: &str, name: &str) -> Result<()> {
        let folder = self.active_list()?;
        let edit = if verb == "rename" {
            crate::store::validate_collection_name(name)?;
            super::personal_tasks::LibraryEdit::RenameFolder {
                folder,
                name: name.into(),
            }
        } else {
            super::personal_tasks::LibraryEdit::DeleteFolder(folder)
        };
        self.submit_library_edit(edit)
    }

    fn folder_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "folder" if argument.is_empty() => {
                self.begin_input(InputKind::NewList, String::new());
            }
            "folder" => {
                self.save_folder(argument, None)?;
                self.status = format!(
                    "Folder created · {argument} · a organizes articles"
                );
            }
            "save" => {
                let article =
                    self.current().map(|article| article.id).ok_or_else(
                        || anyhow::anyhow!("Select an article first"),
                    )?;
                let mut input = Input::new(
                    InputKind::AddToList(article),
                    argument.to_owned(),
                    String::new(),
                );
                return self.apply_input(&mut input).map(Some);
            }
            "folders" => self.open_picker(super::PickerKind::Folders)?,
            "organize" => {
                return self.perform(super::WorkspaceCommand::Lists).map(Some);
            }
            "favorites" => self.set_view(View::Starred)?,
            "favorite" => self.toggle_favorite()?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    pub(super) fn toggle_favorite(&mut self) -> Result<()> {
        if let Some(article) = self.current() {
            self.submit_article_edit(
                [article.id].into(),
                crate::model::BulkEdit::Favorite(!article.starred),
            )?;
            if self.personal_writes.is_none() {
                self.reload()?;
            }
            self.status = "Favorite updated".into();
        }
        Ok(())
    }

    pub(super) fn toggle_read(&mut self) -> Result<()> {
        if let Some(article) = self.current() {
            self.set_read_state(!article.unread)?;
        }
        Ok(())
    }

    fn set_read_state(&mut self, unread: bool) -> Result<()> {
        if let Some(article) = self.current() {
            self.auto_read_manual_state(article.id);
        }
        let article = self.current().ok_or_else(|| {
            anyhow::anyhow!("Select an article to change its read state")
        })?;
        if article.unread == unread {
            self.status = if unread {
                "Article is already unread".into()
            } else {
                "Article is already in Read".into()
            };
            return Ok(());
        }
        let id = article.id;
        self.submit_read_state(std::iter::once(id).collect(), unread)?;
        self.retained_article = None;
        if self.personal_writes.is_none() {
            self.reload()?;
        }
        self.status = if unread {
            "Marked unread · returned to Inbox · u undo".into()
        } else {
            "Marked read · saved in Read · u undo".into()
        };
        Ok(())
    }

    pub(super) fn remove_from_list(&mut self) -> Result<()> {
        let list = self.active_list()?;
        if let Some(article) = self.current() {
            self.submit_library_edit(
                super::personal_tasks::LibraryEdit::RemoveMember {
                    article: article.id,
                    folder: list,
                },
            )?;
            self.status = "Article removed from this list".into();
        }
        Ok(())
    }

    fn active_list(&self) -> Result<i64> {
        if let View::Collection(id) = self.view {
            return Ok(id);
        }
        bail!("Select a personal folder in the library first")
    }

    pub(super) fn edit_comment(&mut self, existing: bool) -> Result<()> {
        self.guard_stale_command("note")?;
        let Some(article) = self.current() else {
            return Ok(());
        };
        let article = article.id;
        let selected = existing
            .then(|| self.comments.get(self.comment_selected))
            .flatten();
        if existing && selected.is_none() {
            self.status = "Select an existing note to edit".into();
            return Ok(());
        }
        let comment = selected.map(|comment| comment.id);
        let text =
            selected.map_or_else(String::new, |comment| comment.body.clone());
        self.ensure_no_recovery(&crate::model::DraftTarget::Note {
            article,
            note: comment,
        })?;
        self.begin_input(InputKind::Comment { article, comment }, text);
        Ok(())
    }

    pub(super) fn select_comment(&mut self, forward: bool) {
        if self.comments.is_empty() {
            return;
        }
        self.focus = super::Focus::Reader;
        self.comment_selected = if forward {
            (self.comment_selected + 1) % self.comments.len()
        } else {
            (self.comment_selected + self.comments.len() - 1)
                % self.comments.len()
        };
        self.status = format!(
            "Comment {}/{} · :edit-note · :delete-comment remove",
            self.comment_selected + 1,
            self.comments.len(),
        );
        let marker = format!("[{}] ", self.comment_selected + 1);
        let row = self
            .reader_lines
            .iter()
            .position(|line| line.starts_with(&marker))
            .unwrap_or(self.scroll);
        self.reader_cursor = row;
        self.reader_column = 0;
        self.scroll = row;
        self.clamp_scroll();
    }

    fn unsubscribe(&mut self) -> Result<()> {
        let View::Feed(id) = self.view else {
            bail!("Select a feed in the library first");
        };
        self.remove_destination(View::Feed(id))
    }

    pub(super) fn open_url(&self) -> Action {
        self.current()
            .filter(|article| !article.url.is_empty())
            .map_or(Action::None, |article| Action::Open(article.url.clone()))
    }
}

fn command_parts(text: &str) -> (&str, &str) {
    let (verb, argument) = text
        .trim()
        .split_once(char::is_whitespace)
        .unwrap_or((text.trim(), ""));
    let verb =
        super::completion::resolve(verb).map_or(verb, |command| command.name);
    (verb, argument.trim())
}
