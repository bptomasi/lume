//! Explicit application operations and the small ex command vocabulary.

use anyhow::{Result, bail};

use super::{Action, App, Input, InputKind, View};

impl App {
    pub(super) fn apply_input(&mut self, input: &Input) -> Result<Action> {
        match &input.kind {
            InputKind::Search => return Ok(Action::None),
            InputKind::ReaderSearch => {
                self.search_reader(&input.text)?;
                return Ok(Action::None);
            }
            InputKind::StudyCard { article, comment } => {
                self.save_study_card(*article, *comment, &input.text)?;
            }
            InputKind::Command => return self.command(&input.text),
            InputKind::NewList => {
                self.store.create_collection(&input.text)?;
                self.status =
                    "Folder created · a organizes the selected article".into();
            }
            InputKind::AddToList(article) => {
                let list = self.store.create_collection(&input.text)?;
                self.store.add_member(list, *article)?;
                self.status = format!("Saved to {}", input.text.trim());
            }
            InputKind::Highlight { article, passage } => {
                self.store.save_passage(*article, passage, &input.text)?;
                self.status = "Highlight and note saved".into();
            }
            InputKind::Discover
            | InputKind::Import
            | InputKind::ExportArticle(_)
            | InputKind::ExportNotebook
            | InputKind::ExportOpml => return self.exchange_input(input),
            InputKind::Comment { article, comment } => {
                self.store.save_comment(*article, *comment, &input.text)?;
                self.status = "Private comment saved".into();
            }
        }
        self.reload()?;
        Ok(Action::None)
    }

    pub(crate) fn command(&mut self, text: &str) -> Result<Action> {
        let (verb, argument) = command_parts(text);
        if let Some(action) = self.layout_commands(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.panel_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.extraction_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.reader_tools_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.organization_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.study_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.settings_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.document_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.exchange_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.workspace_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.folder_command(verb, argument)? {
            return Ok(action);
        }
        if let Some(action) = self.article_command(verb)? {
            return Ok(action);
        }
        self.library_command(verb, argument)
    }

    fn library_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Action> {
        match verb {
            "q" | "quit" => return Ok(Action::Quit),
            "r" | "refresh" => return Ok(Action::Refresh),
            "feed" => {
                self.store.subscribe(argument)?;
                self.reload()?;
                self.status = "Feed added; collecting in the background".into();
                return Ok(Action::Refresh);
            }
            "rename" => self
                .store
                .rename_collection(self.active_list()?, argument)?,
            "delete-folder" => {
                self.store.delete_collection(self.active_list()?)?
            }
            "delete-comment" => self.delete_selected_comment()?,
            "unfeed" => self.unsubscribe()?,
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
                self.store.create_collection(argument)?;
                self.reload()?;
                self.status = format!(
                    "Folder created · {argument} · a organizes articles"
                );
            }
            "save" => {
                let article =
                    self.current().map(|article| article.id).ok_or_else(
                        || anyhow::anyhow!("Select an article first"),
                    )?;
                let input = Input {
                    kind: InputKind::AddToList(article),
                    text: argument.to_owned(),
                    previous_search: String::new(),
                    editor: super::editor::textarea(argument),
                    completion: 0,
                };
                return self.apply_input(&input).map(Some);
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
            self.store.set_starred(article.id, !article.starred)?;
            self.reload()?;
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
        self.store.set_unread(article.id, unread)?;
        self.retained_article = None;
        self.reload()?;
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
            self.store.remove_member(list, article.id)?;
            self.reload()?;
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

    pub(super) fn edit_comment(&mut self, existing: bool) {
        let Some(article) = self.current() else {
            return;
        };
        let article = article.id;
        let selected = existing
            .then(|| self.comments.get(self.comment_selected))
            .flatten();
        let comment = selected.map(|comment| comment.id);
        let text =
            selected.map_or_else(String::new, |comment| comment.body.clone());
        self.begin_input(InputKind::Comment { article, comment }, text);
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
        self.scroll = self
            .reader_lines
            .iter()
            .position(|line| line.starts_with(&marker))
            .unwrap_or(self.scroll);
        self.clamp_scroll();
    }

    fn delete_selected_comment(&mut self) -> Result<()> {
        let Some(article) = self.current() else {
            bail!("Select an article first");
        };
        let Some(comment) = self.comments.get(self.comment_selected) else {
            bail!("No comment selected");
        };
        self.store.delete_comment(article.id, comment.id)
    }

    fn unsubscribe(&mut self) -> Result<()> {
        let View::Feed(id) = self.view else {
            bail!("Select a feed in the library first");
        };
        self.store.unsubscribe(id)?;
        self.view = View::All;
        Ok(())
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
