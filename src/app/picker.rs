//! Bounded selectors share keyboard behavior while retaining typed operations.

use std::collections::BTreeSet;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::{model::ArticleQuery, source::discovery::Candidate};

use super::{Action, App, Mode, View};

/// Operations exposed through both the palette and Space menu.
#[derive(Clone, Copy, Debug)]
pub enum WorkspaceCommand {
    /// Expand the article to the entire terminal.
    Fullscreen,
    /// Toggle centered reading.
    Zen,
    /// Toggle library sidebar.
    Library,
    /// Toggle headline panel.
    Headlines,
    /// Toggle the article reader independently of other panes.
    ReaderPane,
    /// Toggle source-ordered article annotations.
    Annotations,
    /// Toggle headline density.
    Compact,
    /// Read source-linked annotations.
    Notebook,
    /// Open the local, cached recommendation shortlist.
    Recommendations,
    /// Open the explicit reading queue.
    Queue,
    /// Search all articles.
    Articles,
    /// Choose multiple personal lists.
    Lists,
    /// Create an empty collection.
    NewList,
    /// Edit typed saved-search criteria.
    Smart,
    /// Find advertised feed links.
    Discover,
    /// Import OPML subscriptions.
    Import,
    /// Export the selected article and notes.
    ExportArticle,
    /// Export every annotation.
    ExportNotebook,
    /// Export subscription URLs.
    ExportOpml,
    /// Choose a content link.
    Links,
    /// Reverse a personal edit.
    Undo,
    /// Collect subscriptions in the background.
    Refresh,
    /// Fetch the selected article using its original URL.
    FetchArticle,
    /// Alternate between cached Full and publisher Feed content.
    ToggleContent,
}

/// Selector-specific state; list changes commit together when Enter is pressed.
#[derive(Clone, Debug)]
pub enum PickerKind {
    /// Choose a workspace operation.
    Commands,
    /// Search all articles.
    Articles,
    /// Browse folders independently of the selected article.
    Folders,
    /// Choose multiple personal lists.
    Lists {
        /// Captured article being organized.
        article: i64,
        /// Pending memberships, committed together.
        checked: BTreeSet<i64>,
        /// New folder names staged locally until confirmation.
        names: BTreeSet<String>,
    },
    /// Choose a content link.
    Links,
    /// Choose an advertised subscription.
    Sources,
    /// Read-only diagnostics or a captured extraction-rule confirmation.
    Inspection,
}

/// Captured operation, independent of background refresh selection changes.
#[derive(Clone, Debug)]
pub enum ChoiceAction {
    /// Captured notebook, type or reference action.
    Note(super::NoteAction),
    /// Execute a workspace operation.
    Run(WorkspaceCommand),
    /// Open a captured article identity.
    JumpArticle(i64),
    /// Change a captured article's Trash state despite background reloads.
    SetDeleted {
        /// Captured article identity.
        article: i64,
        /// Move to Trash when true; restore when false.
        deleted: bool,
    },
    /// Open a destination.
    SetView(View),
    /// Subscribe to a captured discovered feed.
    Subscribe(Candidate),
    /// Open a validated HTTP(S) URL.
    OpenUrl(String),
    /// Toggle captured list membership.
    Membership(i64),
    /// A staged folder name with no fabricated database identity.
    NamedMembership(String),
    /// Save exactly one previewed source rule and its captured article cache.
    SaveRule(Box<crate::model::RulePreview>),
    /// Jump to a captured article/source/revision passage.
    ReaderJump(super::ReaderLocation),
    /// Captured study operation with its own narrow target contract.
    Study(super::StudyAction),
}

/// One selectable row and its searchable context.
#[derive(Clone, Debug)]
pub struct Choice {
    /// Visible row label.
    pub title: String,
    /// Searchable description and preview.
    pub detail: String,
    /// Typed operation performed on confirmation.
    pub action: ChoiceAction,
}

/// Selector cache; rendering performs no database work.
#[derive(Clone, Debug)]
pub struct Picker {
    /// Operation and pending selector state.
    pub kind: PickerKind,
    /// Bounded literal search input.
    pub query: String,
    /// Active row within the filtered choices.
    pub selected: usize,
    /// Filtered, bounded selection rows.
    pub choices: Vec<Choice>,
    /// Cached context for the active row.
    pub preview: String,
    pub(crate) candidates: Vec<Choice>,
}

impl App {
    pub(super) fn open_picker(&mut self, kind: PickerKind) -> Result<()> {
        let candidates = self.picker_candidates(&kind);
        self.mode = Mode::Picker(Box::new(Picker {
            kind,
            query: String::new(),
            selected: 0,
            choices: Vec::new(),
            preview: String::new(),
            candidates,
        }));
        self.update_picker()
    }

    fn picker_candidates(&self, kind: &PickerKind) -> Vec<Choice> {
        match kind {
            PickerKind::Commands => {
                let mut choices = command_choices();
                choices.extend(self.deletion_choice());
                choices.extend(self.navigation.iter().map(|item| Choice {
                    title: format!("Go to {}", item.title),
                    detail: format!("{} · {} articles", item.group, item.count),
                    action: ChoiceAction::SetView(item.view.clone()),
                }));
                choices
            }
            PickerKind::Folders => self.folder_choices(),
            PickerKind::Lists { .. } => self
                .library
                .collections
                .iter()
                .map(|list| Choice {
                    title: list.name.clone(),
                    detail: "Space select · Enter save · Ctrl-n create".into(),
                    action: ChoiceAction::Membership(list.id),
                })
                .collect(),
            PickerKind::Links => self
                .links
                .iter()
                .map(|(title, url)| Choice {
                    title: title.clone(),
                    detail: url.clone(),
                    action: ChoiceAction::OpenUrl(url.clone()),
                })
                .collect(),
            _ => Vec::new(),
        }
    }

    fn folder_choices(&self) -> Vec<Choice> {
        let mut choices = vec![Choice {
            title: "New folder".into(),
            detail: "Create an empty personal article folder".into(),
            action: ChoiceAction::Run(WorkspaceCommand::NewList),
        }];
        choices.extend(self.library.collections.iter().map(|folder| Choice {
            title: folder.name.clone(),
            detail: "Open this personal article folder".into(),
            action: ChoiceAction::SetView(View::Collection(folder.id)),
        }));
        choices
    }

    pub(super) fn update_picker(&mut self) -> Result<()> {
        let Mode::Picker(picker) = &mut self.mode else {
            return Ok(());
        };
        if matches!(picker.kind, PickerKind::Articles) {
            picker.choices = article_choices(&self.store, &picker.query)?;
        } else {
            let terms = picker.query.to_lowercase();
            picker.choices = picker
                .candidates
                .iter()
                .filter(|choice| {
                    let text = format!("{} {}", choice.title, choice.detail)
                        .to_lowercase();
                    terms.split_whitespace().all(|term| text.contains(term))
                })
                .cloned()
                .collect();
        }
        picker.selected =
            picker.selected.min(picker.choices.len().saturating_sub(1));
        picker.preview = match picker.choices.get(picker.selected) {
            Some(Choice {
                action: ChoiceAction::JumpArticle(id),
                ..
            }) => self.store.body(*id)?.chars().take(6000).collect(),
            Some(choice) => choice.detail.clone(),
            None => "No matches · refine your search".into(),
        };
        Ok(())
    }

    pub(super) fn picker_key(&mut self, key: KeyEvent) -> Result<Action> {
        if key.code == KeyCode::Esc {
            self.mode = Mode::Normal;
            self.resume_note_editor();
            return Ok(Action::None);
        }
        if key.code == KeyCode::Enter {
            return self.commit_picker();
        }
        let Mode::Picker(picker) = &mut self.mode else {
            return Ok(Action::None);
        };
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('n')
        {
            return self.create_picker_list();
        }
        picker.edit_query(key);
        self.update_picker()?;
        Ok(Action::None)
    }

    fn create_picker_list(&mut self) -> Result<Action> {
        let Mode::Picker(picker) = &mut self.mode else {
            return Ok(Action::None);
        };
        let PickerKind::Lists { names, .. } = &mut picker.kind else {
            return Ok(Action::None);
        };
        let name =
            crate::store::validate_collection_name(&picker.query)?.to_owned();
        if names.insert(name.clone()) {
            picker.candidates.push(Choice {
                title: name.clone(),
                detail: "New folder · Enter commits · Esc cancels".into(),
                action: ChoiceAction::NamedMembership(name),
            });
        }
        picker.query.clear();
        self.update_picker()?;
        Ok(Action::None)
    }

    fn commit_picker(&mut self) -> Result<Action> {
        let Mode::Picker(picker) = &self.mode else {
            return Ok(Action::None);
        };
        if let Some(edit) = membership_edit(picker)? {
            self.submit_library_edit(edit)?;
            self.mode = Mode::Normal;
            self.status = "List memberships admitted · u undo".into();
            return Ok(Action::None);
        }
        let Some(action) = picker
            .choices
            .get(picker.selected)
            .map(|choice| choice.action.clone())
        else {
            return Ok(Action::None);
        };
        let previous = std::mem::take(&mut self.mode);
        match self.apply_choice(action) {
            Ok(action) => Ok(action),
            Err(error) => {
                self.mode = previous;
                Err(error)
            }
        }
    }

    pub(super) fn apply_choice(
        &mut self,
        action: ChoiceAction,
    ) -> Result<Action> {
        match action {
            ChoiceAction::Note(action) => {
                return self.apply_note_action(action);
            }
            ChoiceAction::Run(command) => return self.perform(command),
            ChoiceAction::SetDeleted { article, deleted } => {
                self.change_article_deleted(article, deleted)?;
            }
            ChoiceAction::JumpArticle(id) => self.jump_article(id)?,
            ChoiceAction::ReaderJump(target) => self.jump_reader(target)?,
            ChoiceAction::SetView(view) => self.set_view(view)?,
            ChoiceAction::Subscribe(candidate) => {
                self.store.subscribe(&candidate.url)?;
                self.reload()?;
                return Ok(Action::Refresh);
            }
            ChoiceAction::OpenUrl(url) => return Ok(Action::Open(url)),
            ChoiceAction::Membership(_) | ChoiceAction::NamedMembership(_) => {}
            ChoiceAction::Study(action) => self.study_choice(action)?,
            ChoiceAction::SaveRule(preview) => {
                self.store.confirm_article_rule(&preview)?;
                self.reload()?;
                self.status = "Extraction rule and preview saved".into();
            }
        }
        Ok(Action::None)
    }

    /// Display advertised feed choices after the background request completes.
    ///
    /// # Errors
    /// Returns selector-loading errors.
    pub fn discovered(&mut self, candidates: Vec<Candidate>) -> Result<()> {
        self.open_picker(PickerKind::Sources)?;
        if let Mode::Picker(picker) = &mut self.mode {
            picker.candidates = candidates
                .into_iter()
                .map(|candidate| Choice {
                    title: candidate.title.clone(),
                    detail: candidate.url.clone(),
                    action: ChoiceAction::Subscribe(candidate),
                })
                .collect();
        }
        self.update_picker()
    }
}

fn membership_edit(
    picker: &Picker,
) -> Result<Option<super::personal_tasks::LibraryEdit>> {
    let PickerKind::Lists {
        article,
        checked,
        names,
    } = &picker.kind
    else {
        return Ok(None);
    };
    let mut names = names.clone();
    if picker.choices.is_empty() && !picker.query.trim().is_empty() {
        names.insert(
            crate::store::validate_collection_name(&picker.query)?.into(),
        );
    }
    Ok(Some(super::personal_tasks::LibraryEdit::Memberships {
        article: *article,
        folders: checked.clone(),
        names,
    }))
}

fn toggle_checked(picker: &mut Picker) {
    let PickerKind::Lists { checked, names, .. } = &mut picker.kind else {
        return;
    };
    match picker
        .choices
        .get(picker.selected)
        .map(|choice| &choice.action)
    {
        Some(ChoiceAction::Membership(id)) => {
            if !checked.remove(id) {
                checked.insert(*id);
            }
        }
        Some(ChoiceAction::NamedMembership(name)) if !names.remove(name) => {
            names.insert(name.clone());
        }
        _ => {}
    }
}

fn command_choices() -> Vec<Choice> {
    COMMANDS
        .iter()
        .map(|(title, detail, command)| Choice {
            title: (*title).into(),
            detail: (*detail).into(),
            action: ChoiceAction::Run(*command),
        })
        .collect()
}

use WorkspaceCommand as C;

const COMMANDS: &[(&str, &str, WorkspaceCommand)] = &[
    (
        "For you",
        "Space R · local recommendations · :recommend-why explains",
        WorkspaceCommand::Recommendations,
    ),
    (
        "Fetch full article",
        "Retrieve and cache the original page in the background · :fetch",
        C::FetchArticle,
    ),
    (
        "Switch article source",
        "Alternate Feed / Full article · :content",
        C::ToggleContent,
    ),
    (
        "Fullscreen article",
        "Use the entire terminal · F11 / Z toggle · Esc restores panels",
        C::Fullscreen,
    ),
    (
        "Zen reading",
        "Center the reader and hide navigation",
        C::Zen,
    ),
    (
        "Toggle library",
        "Show or hide subscriptions and lists",
        C::Library,
    ),
    (
        "Toggle headlines",
        "Show or hide the article panel",
        C::Headlines,
    ),
    (
        "Compact headlines",
        "Switch between compact and detailed rows",
        C::Compact,
    ),
    (
        "Notebook",
        "Read your annotations with their source context",
        C::Notebook,
    ),
    ("Read next", "Open your explicit reading queue", C::Queue),
    (
        "Find article",
        "Search all article bodies and annotations",
        C::Articles,
    ),
    (
        "Organize in folders",
        "Select multiple folders, or create one inline",
        C::Lists,
    ),
    (
        "Article annotations",
        "Show notes in source order beside your reading",
        C::Annotations,
    ),
    (
        "Toggle reader pane",
        "Collapse or restore the article reader",
        C::ReaderPane,
    ),
    ("New folder", "Create a personal article folder", C::NewList),
    ("Smart list", "Save or edit typed search criteria", C::Smart),
    (
        "Discover feeds",
        "Find RSS/Atom/JSON feeds advertised by a website",
        C::Discover,
    ),
    (
        "Import OPML",
        "Import subscriptions atomically from a local file",
        C::Import,
    ),
    (
        "Export article",
        "Write article and notes to a new Markdown file",
        C::ExportArticle,
    ),
    (
        "Export notebook",
        "Write all notes and source links to Markdown",
        C::ExportNotebook,
    ),
    (
        "Export OPML",
        "Back up subscriptions to a new OPML file",
        C::ExportOpml,
    ),
    (
        "Article links",
        "Choose a validated link to open in your browser",
        C::Links,
    ),
    ("Undo", "Reverse the last personal edit", C::Undo),
    (
        "Refresh feeds",
        "Collect subscriptions in the background",
        C::Refresh,
    ),
];

impl Picker {
    fn edit_query(&mut self, key: KeyEvent) {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        match key.code {
            KeyCode::Down | KeyCode::Tab => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.choices.len().saturating_sub(1));
            }
            KeyCode::Up | KeyCode::BackTab => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char('j') if control => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.choices.len().saturating_sub(1));
            }
            KeyCode::Char('k') if control => {
                self.selected = self.selected.saturating_sub(1);
            }
            KeyCode::Char(' ')
                if matches!(self.kind, PickerKind::Lists { .. }) =>
            {
                toggle_checked(self);
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c)
                if !c.is_control() && !control && self.query.len() < 4096 =>
            {
                self.query.push(c);
                self.selected = 0;
            }
            _ => {}
        }
    }
}

fn article_choices(
    store: &crate::store::Store,
    text: &str,
) -> Result<Vec<Choice>> {
    Ok(store
        .query_articles(&ArticleQuery {
            view: View::All,
            text: text.to_owned(),
            offset: 0,
            limit: 50,
            retained: None,
        })?
        .articles
        .into_iter()
        .map(|article| Choice {
            title: article.title,
            detail: article.feed,
            action: ChoiceAction::JumpArticle(article.id),
        })
        .collect())
}
