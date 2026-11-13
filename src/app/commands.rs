//! Explicit application operations and the small ex command vocabulary.

use crate::command::Command;

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
        let (word, argument) = command_parts(text);
        let spec = crate::command::resolve(word).ok_or_else(|| {
            anyhow::anyhow!(
                "Unknown command · Tab completes · :help shows the guide"
            )
        })?;
        let command = spec.command;
        // Session writes and web navigation precede article-specific guards.
        let owner = command_owner(command);
        if let CommandOwner::Session(command) = owner {
            anyhow::ensure!(
                argument.is_empty(),
                ":{word} does not accept arguments"
            );
            return self.session_command(command, argument);
        }
        anyhow::ensure!(
            !self.web.active || command.allowed_on_linked_page(),
            "Linked pages are read-only · :save-page adds one to the library"
        );
        if let CommandOwner::Maintenance(command) = owner {
            return self.maintenance_command(command, argument);
        }
        let web_image =
            self.web.active && matches!(owner, CommandOwner::Image(_));
        if !matches!(owner, CommandOwner::Web(_)) && !web_image {
            self.guard_notebook_article_command(command)?;
            self.guard_stale_command(command)?;
        }
        self.dispatch_command(command, argument)
    }

    fn dispatch_auto_read(
        &mut self,
        command: routing::AutoReadCommand,
        argument: &str,
    ) -> Result<Action> {
        match command {
            routing::AutoReadCommand::AutoRead => {
                self.auto_read_command(argument)
            }
        }
    }

    fn dispatch_performance(
        &mut self,
        command: routing::PerformanceCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::PerformanceCommand as C;

        match command {
            C::Performance => self.performance_command(argument),
        }
    }

    fn dispatch_configuration(
        &mut self,
        command: routing::ConfigurationCommand,
        _argument: &str,
    ) -> Result<Action> {
        self.configuration_command(command)
    }

    fn library_command(
        &mut self,
        command: routing::LibraryCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::LibraryCommand as C;

        match command {
            C::Refresh => return Ok(Action::Refresh),
            C::Feed => return self.add_feed(argument),
            C::Rename => self.rename_folder(argument)?,
            C::DeleteFolder => {
                self.submit_library_edit(
                    super::personal_tasks::LibraryEdit::DeleteFolder(
                        self.active_list()?,
                    ),
                )?;
            }
            C::DeleteComment => {
                let note = self.note_target()?;
                self.delete_captured_note(note.article, note.comment.id)?;
                return Ok(Action::None);
            }
            C::Unfeed => {
                self.unsubscribe()?;
                return Ok(Action::None);
            }
            C::Read => {
                self.set_view(View::Read)?;
                return Ok(Action::None);
            }
            C::MarkRead | C::MarkUnread => {
                self.set_read_state(command == C::MarkUnread)?;
                return Ok(Action::None);
            }
            C::Help => {
                self.show_help(argument)?;
                return Ok(Action::None);
            }
        }
        self.reload()?;
        self.status = "Library updated".into();
        Ok(Action::None)
    }

    fn add_feed(&mut self, url: &str) -> Result<Action> {
        self.store.subscribe(url)?;
        self.reload()?;
        self.status = "Feed added; collecting in the background".into();
        Ok(Action::Refresh)
    }

    fn save_folder(&mut self, name: &str, article: Option<i64>) -> Result<()> {
        crate::validation::validate_collection_name(name)?;
        self.submit_library_edit(
            super::personal_tasks::LibraryEdit::CreateFolder {
                name: name.into(),
                article,
            },
        )
    }

    fn rename_folder(&mut self, name: &str) -> Result<()> {
        let folder = self.active_list()?;
        crate::validation::validate_collection_name(name)?;
        self.submit_library_edit(
            super::personal_tasks::LibraryEdit::RenameFolder {
                folder,
                name: name.into(),
            },
        )
    }

    fn folder_command(
        &mut self,
        command: routing::FolderCommand,
        argument: &str,
    ) -> Result<Action> {
        match command {
            routing::FolderCommand::Folder if argument.is_empty() => {
                self.begin_input(InputKind::NewList, String::new());
            }
            routing::FolderCommand::Folder => {
                self.save_folder(argument, None)?;
                self.status = format!(
                    "Folder created · {argument} · a organizes articles"
                );
            }
            routing::FolderCommand::Save => {
                let article =
                    self.current().map(|article| article.id).ok_or_else(
                        || anyhow::anyhow!("Select an article first"),
                    )?;
                let mut input = Input::new(
                    InputKind::AddToList(article),
                    argument.to_owned(),
                    String::new(),
                );
                return self.apply_input(&mut input);
            }
            routing::FolderCommand::Folders => {
                self.open_picker(super::PickerKind::Folders)?
            }
            routing::FolderCommand::Organize => {
                return self.perform(super::WorkspaceCommand::Lists);
            }
            routing::FolderCommand::Favorites => {
                self.set_view(View::Starred)?
            }
            routing::FolderCommand::Favorite => self.toggle_favorite()?,
        }
        Ok(Action::None)
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
        self.guard_stale_command(crate::command::Command::Note)?;
        let Some(article) = self.current() else {
            return Ok(());
        };
        let article = article.id;
        let selected = existing
            .then(|| {
                self.reader
                    .view()
                    .comments
                    .get(self.reader.view().comment_selected)
            })
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
        if self.reader.view().comments.is_empty() {
            return;
        }
        self.focus = super::Focus::Reader;
        let selected = if forward {
            (self.reader.view().comment_selected + 1)
                % self.reader.view().comments.len()
        } else {
            (self.reader.view().comment_selected
                + self.reader.view().comments.len()
                - 1)
                % self.reader.view().comments.len()
        };
        self.reader.select_comment(selected);
        self.status = format!(
            "Comment {}/{} · :edit-note · :delete-comment remove",
            self.reader.view().comment_selected + 1,
            self.reader.view().comments.len(),
        );
        let marker = format!("[{}] ", self.reader.view().comment_selected + 1);
        let row = self
            .reader
            .view()
            .reader_lines
            .iter()
            .position(|line| line.starts_with(&marker))
            .unwrap_or(self.reader.first_row());
        self.reader.point_cursor(row, 0);
        self.reader.reveal_row(row);
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
    (verb, argument.trim())
}

// Generate closed owner-specific commands from the exhaustive routing table.
// Handlers match their own enum exhaustively, so adding a registry identity
// requires both an explicit owner and a corresponding handler branch.
macro_rules! command_routes {
    ($(
        $owner:ident($identity:ident) => $handler:ident [
            $($variant:ident),+ $(,)?
        ]
    ),+ $(,)?) => {
        pub(in crate::app) mod routing {
            $(#[derive(Clone, Copy, PartialEq, Eq)]
            pub(in crate::app) enum $identity {
                $($variant,)+
            })+
        }

        #[derive(Clone, Copy)]
        enum CommandOwner {
            $($owner(routing::$identity),)+
        }

        fn command_owner(command: Command) -> CommandOwner {
            match command {
                $($(Command::$variant =>
                    CommandOwner::$owner(routing::$identity::$variant),)+)+
            }
        }

        impl App {
            fn dispatch_command(
                &mut self,
                command: Command,
                argument: &str,
            ) -> Result<Action> {
                match command_owner(command) {
                    $(CommandOwner::$owner(command) =>
                        self.$handler(command, argument),)+
                }
            }
        }
    };
}

command_routes! {
    Recommendation(RecommendationCommand) => recommendation_command [
        Recommendations, RecommendProfile, Interest, InterestRemove,
        RecommendExclude, RecommendSettings, RecommendFeedback,
        RecommendWhy, Similar,
    ],
    Layout(LayoutCommand) => layout_commands [
        Workspace, LayoutUndo, LayoutRedo, LayoutReset, PanelDock,
        LayoutColumns,
    ],
    Panel(PanelCommand) => panel_command [
        Panel, PanelFocus, PanelClose, PanelMove, PanelSplit, PanelResize,
        Annotations, Reader, Mouse,
    ],
    Organization(OrganizationCommand) => organization_command [
        SelectArticle, SelectPage, ClearSelection, Bulk, Tags,
        Related, Pin, Collapse, MoveUp, MoveDown, Continue,
        SinceLastVisit, Grouped,
    ],
    Server(ServerCommand) => server_command [
        Server, Servers, ServerHistory, Unserver, ServerEnable,
        ServerDisable,
    ],
    Study(StudyCommand) => study_command [
        Review, Cards, Card, ReviewIn, LinkNote, NoteLinks, ExportNotes,
    ],
    Settings(SettingsCommand) => settings_command [
        Settings, Theme, Justify, Bind, Unbind,
    ],
    Exchange(ExchangeCommand) => exchange_command [
        Discover, Import, Export, ExportNotebook, ExportOpml,
    ],
    Workspace(WorkspaceCommand) => workspace_command [
        Zen, Fullscreen, Sidebar, Articles, Compact, Notebook, Queue, Undo,
        Smart, Links, Palette, Editor, DeleteSmart,
    ],
    Dimension(DimensionCommand) => dimension_command [
        Width, FullscreenWidth, SidebarWidth, ArticlesWidth,
    ],
    Folder(FolderCommand) => folder_command [
        Folder, Save, Folders, Organize, Favorites, Favorite,
    ],
    Article(ArticleCommand) => article_command [
        DeleteArticle, RestoreArticle, Trash,
    ],
    Library(LibraryCommand) => library_command [
        Refresh, Feed, Rename, DeleteFolder, DeleteComment, Unfeed, Read,
        MarkRead, MarkUnread, Help,
    ],
    Notebook(NotebookCommand) => notebook_command [
        Notes, NoteSearch, NoteCollection, NoteType, NoteStatus, NotePin,
        NoteTags, HighlightTags, NoteBacklinks,
    ],
    NotebookInspection(NotebookInspectionCommand) => notebook_inspection_command [
        Drafts, ExportNoteFiles, NoteHistory, NoteReanchor, NoteSynthesis,
        NoteRecent, NoteCollectionDelete,
    ],
    Extraction(ExtractionCommand) => extraction_command [
        ExtractPreview, SourceHealth,
    ],
    ReaderTools(ReaderToolsCommand) => reader_tools_command [
        ExplainSearch, Find, FindNext, FindPrev, Note, EditNote, Annotate,
        Toc, Highlights, JumpBack, JumpForward, CopyCode, Tables,
    ],
    Image(ImageCommand) => image_command [
        Image, ImageOpen, Images, ImageProtocol,
    ],
    Document(DocumentCommand) => document_command [
        Fetch, Content, FetchPolicy,
    ],
    Maintenance(MaintenanceCommand) => maintenance_command [
        Offline, Backup, Storage, Session, Retention,
    ],
    Web(WebCommand) => web_command [
        OpenLink, OpenPage, SavePage, LatexFormulas,
    ],
    Configuration(ConfigurationCommand) => dispatch_configuration [
        Config, ConfigReload,
    ],
    Session(SessionCommand) => session_command [
        Write, Wq, Wqa, Quit, SaveStatus,
    ],
    AutoRead(AutoReadCommand) => dispatch_auto_read [
        AutoRead,
    ],
    Performance(PerformanceCommand) => dispatch_performance [
        Performance,
    ],
}

#[cfg(test)]
mod tests;
