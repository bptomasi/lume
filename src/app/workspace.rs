//! Layout operations and a typed smart-list form own their validation.

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_textarea::TextArea;

use crate::model::SmartFilter;

use super::{
    Action, App, Focus, InputKind, Mode, PickerKind, View, WorkspaceCommand,
};

/// Four text fields plus unread/favorite switches, navigated with Tab.
#[derive(Clone, Debug)]
pub struct SmartForm {
    /// Name, full-text terms, optional source and optional age.
    pub fields: Vec<TextArea<'static>>,
    /// Active field, followed by the two boolean switches.
    pub field: usize,
    /// Restrict results to unread articles.
    pub unread: bool,
    /// Restrict results to favorites.
    pub starred: bool,
    /// Grouped source identities retained when editing a saved topic.
    pub feeds: Vec<i64>,
}

impl App {
    pub(super) fn perform(
        &mut self,
        command: WorkspaceCommand,
    ) -> Result<Action> {
        use WorkspaceCommand as C;
        match command {
            C::ReaderPane
            | C::Annotations
            | C::Fullscreen
            | C::Zen
            | C::Library
            | C::Headlines
            | C::Compact => {
                self.layout_command(command)?;
            }
            C::Notebook => {
                self.open_notebook("", None)?;
            }
            C::Recommendations => return self.command("recommendations"),
            C::Queue => self.set_view(View::Queue)?,
            C::Articles => self.open_picker(PickerKind::Articles)?,
            C::Lists => self.open_memberships()?,
            C::NewList => self.begin_input(InputKind::NewList, String::new()),
            C::Smart => self.open_smart(),
            C::Discover => self.begin_input(InputKind::Discover, String::new()),
            C::Import => self.begin_input(InputKind::Import, String::new()),
            C::ExportArticle | C::ExportNotebook | C::ExportOpml => {
                self.begin_workspace_export(command);
            }
            C::Links => self.open_picker(PickerKind::Links)?,
            C::Undo => self.undo()?,
            C::Refresh => return Ok(Action::Refresh),
            C::FetchArticle => return self.command("fetch"),
            C::ToggleContent => return self.command("content"),
        }
        Ok(Action::None)
    }

    fn open_memberships(&mut self) -> Result<()> {
        let article = self
            .current()
            .map(|article| article.id)
            .ok_or_else(|| anyhow::anyhow!("Select an article to organize"))?;
        self.open_picker(PickerKind::Lists {
            article,
            checked: self.store.article_memberships(article)?,
        })
    }

    fn layout_command(&mut self, command: WorkspaceCommand) -> Result<()> {
        use WorkspaceCommand as C;
        match command {
            C::ReaderPane => return self.toggle_pane(Focus::Reader),
            C::Annotations => return self.toggle_pane(Focus::Annotations),
            C::Fullscreen => return self.toggle_fullscreen(),
            _ => {}
        }
        if self.workspace.fullscreen {
            self.toggle_fullscreen()?;
            if matches!(command, C::Zen) {
                return Ok(());
            }
        }
        match command {
            C::Zen => {
                self.workspace.zen = !self.workspace.zen;
                self.workspace.show_reader = true;
                self.focus = Focus::Reader;
            }
            C::Library => {
                self.workspace.show_library = !self.workspace.show_library
            }
            C::Headlines => {
                self.workspace.show_articles = !self.workspace.show_articles
            }
            C::Compact => self.workspace.compact = !self.workspace.compact,
            _ => {}
        }
        self.ensure_visible_focus();
        self.save_session()?;
        self.status = "Layout saved".into();
        Ok(())
    }

    pub(super) fn ensure_visible_focus(&mut self) {
        if self.workspace.fullscreen
            || self.workspace.zen && self.workspace.show_reader
        {
            self.focus = Focus::Reader;
            return;
        }
        if !self.workspace.pane_visible(self.focus) {
            let expanded = self.expanded_panes();
            self.focus = if expanded.contains(&Focus::Reader) {
                Focus::Reader
            } else {
                expanded.first().copied().unwrap_or(Focus::Reader)
            };
        }
    }

    fn toggle_fullscreen(&mut self) -> Result<()> {
        if !self.workspace.fullscreen {
            ensure!(self.current().is_some(), "Select an article first");
            self.fullscreen_focus = Some(self.focus);
            self.workspace.show_reader = true;
            self.focus = Focus::Reader;
        } else {
            self.focus = self.fullscreen_focus.take().unwrap_or(Focus::Reader);
        }
        self.workspace.fullscreen = !self.workspace.fullscreen;
        self.ensure_visible_focus();
        self.save_session()?;
        if !self.screen.is_empty() {
            self.resize_workspace(self.screen);
        }
        self.status = if self.workspace.fullscreen {
            "Article expanded".into()
        } else {
            "Reading layout restored".into()
        };
        Ok(())
    }

    pub(super) fn undo(&mut self) -> Result<()> {
        if let Some(writer) = &mut self.note_tasks {
            writer.submit(super::note_tasks::Task::Undo)?;
            self.status = "Undoing personal edit…".into();
            return Ok(());
        }
        match self.store.undo_effect()? {
            crate::model::UndoEffect::Unchanged => {
                self.status = "No personal edits to undo".into()
            }
            crate::model::UndoEffect::Notes(article) => {
                self.refresh_notes(article)?;
                self.status = "Note edit undone".into();
            }
            crate::model::UndoEffect::Library => {
                self.reload()?;
                self.status = "Personal edit undone".into();
            }
        }
        Ok(())
    }

    pub(super) fn toggle_queue(&mut self) -> Result<()> {
        if let Some(article) = self.current() {
            self.store.set_queued(article.id, !article.queued)?;
            self.reload()?;
            self.status = "Read-next queue updated · u undo".into();
        }
        Ok(())
    }

    pub(super) fn leader_key(&mut self, key: KeyEvent) -> Result<Action> {
        use WorkspaceCommand as C;
        self.mode = Mode::Normal;
        let command = match key.code {
            KeyCode::Char('R') => {
                return self.command("recommendations");
            }
            KeyCode::Char('/') => {
                self.begin_global_note_search()?;
                return Ok(Action::None);
            }
            KeyCode::Char('p') | KeyCode::Char(' ') => {
                self.open_picker(PickerKind::Commands)?;
                return Ok(Action::None);
            }
            KeyCode::Char('f') => C::Articles,
            KeyCode::Char('l') => C::Lists,
            KeyCode::Char('n') => C::Notebook,
            KeyCode::Char('q') => C::Queue,
            KeyCode::Char('z') => C::Zen,
            KeyCode::Char('Z') => C::Fullscreen,
            KeyCode::Char('b') => C::Library,
            KeyCode::Char('a') => C::Headlines,
            KeyCode::Char('t') => C::Annotations,
            KeyCode::Char('v') => C::ReaderPane,
            KeyCode::Char('c') => C::Compact,
            KeyCode::Char('s') => C::Smart,
            KeyCode::Char('d') => C::Discover,
            KeyCode::Char('i') => C::Import,
            KeyCode::Char('e') => C::ExportArticle,
            KeyCode::Char('o') => C::Links,
            KeyCode::Char('u') => C::Undo,
            KeyCode::Char('r') => C::Refresh,
            _ => return Ok(Action::None),
        };
        self.perform(command)
    }

    fn open_smart(&mut self) {
        let current = match self.view {
            View::Smart(id) => {
                self.library.smart_lists.iter().find(|list| list.id == id)
            }
            _ => None,
        };
        let filter =
            current.map(|list| list.filter.clone()).unwrap_or_default();
        let fields = [
            current.map_or("", |list| list.name.as_str()).to_owned(),
            filter.text,
            filter.feed.map_or_else(String::new, |id| {
                self.library
                    .feeds
                    .iter()
                    .find(|feed| feed.id == id)
                    .map_or_else(|| id.to_string(), |feed| feed.url.clone())
            }),
            filter
                .days
                .map_or_else(String::new, |days| days.to_string()),
        ]
        .iter()
        .map(|text| super::editor::textarea(text))
        .collect();
        self.mode = Mode::Smart(Box::new(SmartForm {
            fields,
            field: 0,
            unread: filter.unread,
            starred: filter.starred,
            feeds: filter.feeds,
        }));
    }

    pub(super) fn smart_key(&mut self, key: KeyEvent) -> Result<Action> {
        let control = key.modifiers.contains(KeyModifiers::CONTROL);
        if key.code == KeyCode::Esc {
            self.mode = Mode::Normal;
            return Ok(Action::None);
        }
        if control && key.code == KeyCode::Char('s') {
            self.save_smart()?;
            return Ok(Action::None);
        }
        let Mode::Smart(form) = &mut self.mode else {
            return Ok(Action::None);
        };
        if control && key.code == KeyCode::Char(' ') && form.field == 1 {
            if let Some(completed) =
                crate::search::complete(&form.fields[1].lines().join(" "))
            {
                form.fields[1] = super::editor::textarea(&completed);
            }
            return Ok(Action::None);
        }
        match key.code {
            KeyCode::Tab | KeyCode::Enter => form.field = (form.field + 1) % 6,
            KeyCode::BackTab => form.field = (form.field + 5) % 6,
            KeyCode::Char(' ') if form.field == 4 => form.unread = !form.unread,
            KeyCode::Char(' ') if form.field == 5 => {
                form.starred = !form.starred
            }
            _ if form.field < 4 => {
                let field = &mut form.fields[form.field];
                if field.lines()[0].len() < 4096
                    || key.code == KeyCode::Backspace
                {
                    field.input(key);
                }
            }
            _ => {}
        }
        Ok(Action::None)
    }

    fn save_smart(&mut self) -> Result<()> {
        let Mode::Smart(form) = &self.mode else {
            return Ok(());
        };
        let values = form
            .fields
            .iter()
            .map(|field| field.lines().join(" "))
            .collect::<Vec<_>>();
        let filter = SmartFilter {
            text: values[1].clone(),
            unread: form.unread,
            starred: form.starred,
            feed: self.smart_source(&values[2])?,
            feeds: form.feeds.clone(),
            days: optional_number(&values[3])?,
        };
        self.store.save_smart_list(&values[0], &filter)?;
        self.mode = Mode::Normal;
        self.reload()?;
        self.status = "Smart list saved".into();
        Ok(())
    }
    fn smart_source(&self, value: &str) -> Result<Option<i64>> {
        let value = value.trim();
        if value.is_empty() {
            return Ok(None);
        }
        let matches = self
            .library
            .feeds
            .iter()
            .filter(|feed| {
                feed.title.eq_ignore_ascii_case(value) || feed.url == value
            })
            .collect::<Vec<_>>();
        if let [feed] = matches.as_slice() {
            return Ok(Some(feed.id));
        }
        ensure!(
            matches.is_empty(),
            "Several sources use this name; enter its feed URL"
        );
        optional_number(value).map_err(|_| {
            anyhow::anyhow!("Enter an exact source name or feed URL")
        })
    }
}

fn optional_number<T: std::str::FromStr>(text: &str) -> Result<Option<T>>
where
    T::Err: std::fmt::Display,
{
    if text.trim().is_empty() {
        return Ok(None);
    }
    text.trim()
        .parse()
        .map(Some)
        .map_err(|error| anyhow::anyhow!("Invalid number: {error}"))
}

impl App {
    pub(super) fn workspace_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        use WorkspaceCommand as C;
        let command = match verb {
            "zen" => C::Zen,
            "fullscreen" => C::Fullscreen,
            "sidebar" => C::Library,
            "articles" => C::Headlines,
            "compact" => C::Compact,
            "notebook" => C::Notebook,
            "queue" => C::Queue,
            "undo" => C::Undo,
            "smart" => C::Smart,
            "links" => C::Links,
            "palette" => {
                self.open_picker(PickerKind::Commands)?;
                return Ok(Some(Action::None));
            }
            "editor" => {
                self.begin_reader_note()?;
                return Ok(Some(Action::ExternalEditor));
            }
            "width" | "fullscreen-width" | "sidebar-width"
            | "articles-width" => {
                self.set_dimension(verb, argument)?;
                return Ok(Some(Action::None));
            }
            "delete-smart" => {
                let View::Smart(id) = self.view else {
                    anyhow::bail!("Select a smart list first");
                };
                self.store.delete_smart_list(id)?;
                self.set_view(View::All)?;
                self.reload()?;
                return Ok(Some(Action::None));
            }
            _ => return Ok(None),
        };
        self.perform(command).map(Some)
    }

    fn set_dimension(&mut self, name: &str, value: &str) -> Result<()> {
        let width = value.parse::<u16>()?;
        let mut workspace = self.workspace.clone();
        match name {
            "width" => workspace.reading_width = width,
            "fullscreen-width" => workspace.fullscreen_width = width,
            "sidebar-width" => workspace.library_width = width,
            "articles-width" => workspace.article_width = width,
            _ => {}
        }
        crate::store::validate_workspace_dimensions(&workspace)?;
        self.workspace = workspace;
        self.persist_preferences()?;
        if !self.screen.is_empty() {
            self.resize_workspace(self.screen);
        }
        Ok(())
    }
}
