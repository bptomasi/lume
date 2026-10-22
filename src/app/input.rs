//! Vim navigation and modal dispatch keep editing keys out of normal mode.

use crate::model::View;
use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

use super::{
    Action, App, Focus, InputKind, Mode, PickerKind, WorkspaceCommand,
};

impl App {
    pub(super) fn handle_key(&mut self, key: KeyEvent) -> Result<Action> {
        if self.ignored_key(key) {
            return Ok(Action::None);
        }
        if !matches!(key.code, KeyCode::Char('d') | KeyCode::Esc) {
            self.note_marks.delete = None;
        }
        self.cancel_pointer();
        if self.stale_reader_key(key) {
            return Ok(Action::None);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL)
            && key.code == KeyCode::Char('c')
        {
            return Ok(Action::Quit);
        }
        if self.inspection_key(key) {
            return Ok(Action::None);
        }
        let action = match &self.mode {
            Mode::Web => self.web_key(key),
            Mode::Help => {
                self.help_key(key);
                Ok(Action::None)
            }
            Mode::Input(_) => self.input_key(key),
            Mode::Picker(_) => self.picker_key(key),
            Mode::Leader => self.leader_key(key),
            Mode::Smart(_) => self.smart_key(key),
            Mode::Document(_) => self.document_key(key),
            Mode::Settings(_) => self.settings_key(key),
            Mode::Image(_) => self.image_inspector_keypress(key),
            Mode::Normal if self.notebook.is_some() => self.notebook_key(key),
            Mode::Normal => self.normal_key(key),
        };
        self.restore_web_mode();
        action
    }

    fn stale_reader_key(&mut self, key: KeyEvent) -> bool {
        let blocked = matches!(self.mode, Mode::Normal)
            && self.notebook.is_none()
            && self.reader_stale()
            && matches!(self.focus, Focus::Reader | Focus::Annotations)
            && !matches!(
                key.code,
                KeyCode::Esc
                    | KeyCode::Tab
                    | KeyCode::BackTab
                    | KeyCode::F(11)
                    | KeyCode::Char('q' | ':' | '?' | 'Z' | 'z')
            )
            && !preview_motion(key.code)
            && !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
            && !self.panel_prefix;
        if blocked {
            self.status = "Loading selection… · article actions paused".into();
        }
        blocked
    }

    fn ignored_key(&self, key: KeyEvent) -> bool {
        if key.kind == KeyEventKind::Release
            || matches!(key.code, KeyCode::Modifier(_))
        {
            return true;
        }
        matches!(self.mode, Mode::Normal | Mode::Web)
            && key.kind == KeyEventKind::Repeat
            && matches!(
                key.code,
                KeyCode::Char('v' | 'V' | 'y' | 's' | 'D') | KeyCode::Delete
            )
    }

    fn inspection_key(&mut self, key: KeyEvent) -> bool {
        if !self.note_inspection_pending() || !matches!(self.mode, Mode::Normal)
        {
            return false;
        }
        if key.code == KeyCode::Esc {
            self.cancel_note_inspection();
        }
        true
    }

    fn normal_key(&mut self, key: KeyEvent) -> Result<Action> {
        if let Some(action) = self.normal_prefix_action(key)? {
            return Ok(action);
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            self.note_marks.delete = None;
            return self.control_key(key.code);
        }
        if let Some(action) = self.reader_link_key(key)? {
            return Ok(action);
        }
        if self.note_key(key)? {
            return Ok(Action::None);
        }
        if let Some(action) = self.annotation_key(key.code)? {
            return Ok(action);
        }
        if self.article_preparing()
            && self.focus == Focus::Reader
            && matches!(key.code, KeyCode::Char('v' | 'V'))
        {
            return Ok(Action::None);
        }
        if self.reader_vim_key(key.code)? {
            return Ok(Action::None);
        }
        if let Some(action) = self.passage_key(key.code)? {
            return Ok(action);
        }
        if self.navigation_key(key.code)? {
            return Ok(Action::None);
        }
        self.workspace_key(key.code)
    }

    pub(super) fn normal_prefix_action(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<Action>> {
        if self.panel_prefix && self.panel_layout_key(key)? {
            return Ok(Some(Action::None));
        }
        if let Some(action) = self.bound_key(key)? {
            return Ok(Some(action));
        }
        if self.panel_layout_key(key)? {
            return Ok(Some(Action::None));
        }
        Ok(None)
    }

    fn workspace_key(&mut self, code: KeyCode) -> Result<Action> {
        match code {
            KeyCode::Char('q') => return Ok(Action::Quit),
            KeyCode::Char('r') => return Ok(Action::Refresh),
            KeyCode::Char('?') => self.mode = Mode::Help,
            KeyCode::Char(' ') => self.mode = Mode::Leader,
            KeyCode::Char('/') => {
                self.begin_context_search()?;
            }
            KeyCode::Char(':') => {
                self.begin_input(InputKind::Command, String::new())
            }
            KeyCode::Char('o') => return Ok(self.open_url()),
            KeyCode::Char('s') => self.toggle_queue()?,
            KeyCode::F(11) | KeyCode::Char('Z') => {
                self.perform(WorkspaceCommand::Fullscreen)?;
            }
            KeyCode::Char('z') => {
                self.perform(WorkspaceCommand::Zen)?;
            }
            KeyCode::Char('u') => self.undo()?,
            code => self.personal_key(code)?,
        }
        Ok(Action::None)
    }

    fn begin_context_search(&mut self) -> Result<()> {
        if self.focus == Focus::Reader {
            self.begin_reader_search();
        } else if self.focus == Focus::Library {
            self.begin_library_search();
        } else {
            if self.focus != Focus::Articles {
                self.focus_pane(Focus::Articles)?;
            }
            self.begin_input(InputKind::Search, self.search.clone());
        }
        Ok(())
    }

    fn bound_key(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        let Some(command) = self
            .workspace
            .keybindings
            .get(&super::key_name(key))
            .cloned()
        else {
            return Ok(None);
        };
        if key.kind == KeyEventKind::Repeat {
            return Ok(Some(Action::None));
        }
        self.vim.clear_input();
        self.command(&command).map(Some)
    }

    fn enter_navigation(&mut self) -> Result<()> {
        if self.focus == Focus::Reader && self.captured_image().is_ok() {
            return self.open_image_inspector();
        }
        if self.focus != Focus::Library {
            return self.open_reader();
        }
        if self.view == View::Annotated {
            self.open_notebook("", None)?;
            return Ok(());
        }
        if let View::Group(group) = &self.view {
            let group = group.clone();
            self.toggle_navigation_group(match group.as_str() {
                "FEEDS" => "feeds",
                "SMART LISTS" => "lists",
                "YOUR FOLDERS" => "folders",
                _ => "pinned",
            })?;
        } else {
            self.focus = Focus::Articles;
        }
        Ok(())
    }

    fn personal_key(&mut self, code: KeyCode) -> Result<()> {
        match code {
            KeyCode::Char('N') => {
                self.begin_input(InputKind::NewList, String::new())
            }
            KeyCode::Char('f') => self.toggle_favorite()?,
            KeyCode::Char('m') => self.toggle_read()?,
            KeyCode::Char('a') => {
                self.perform(WorkspaceCommand::Lists)?;
            }
            KeyCode::Char('d') if self.selection.is_none() => {
                self.remove_from_list()?
            }
            KeyCode::Char('D') | KeyCode::Delete
                if self.focus != Focus::Library && self.selection.is_none() =>
            {
                self.set_selected_deleted(true)?;
            }
            KeyCode::Char('n') => self.annotate_selection()?,
            KeyCode::Char('e') => self.edit_comment(true)?,
            KeyCode::Char(']') => self.select_comment(true),
            KeyCode::Char('[') => self.select_comment(false),
            _ => {}
        }
        Ok(())
    }

    fn navigation_key(&mut self, code: KeyCode) -> Result<bool> {
        let previous_g = std::mem::take(&mut self.pending_g);
        match code {
            KeyCode::Char('j') | KeyCode::Down => {
                self.move_selection(true, 1)?
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.move_selection(false, 1)?
            }
            KeyCode::Char('h') | KeyCode::Left | KeyCode::BackTab => {
                self.change_focus(false)
            }
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Tab => {
                self.change_focus(true)
            }
            KeyCode::Enter => self.enter_navigation()?,
            KeyCode::Char('G') | KeyCode::End => {
                self.move_selection(true, usize::MAX)?
            }
            KeyCode::Home => self.move_selection(false, usize::MAX)?,
            KeyCode::Char('g') if previous_g => {
                self.move_selection(false, usize::MAX)?
            }
            KeyCode::Char('g') => self.pending_g = true,
            KeyCode::Esc => self.escape_reader()?,
            KeyCode::PageDown => {
                self.move_selection(true, self.reader_height)?
            }
            KeyCode::PageUp => {
                self.move_selection(false, self.reader_height)?
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    fn escape_reader(&mut self) -> Result<()> {
        let reader_search =
            self.focus == Focus::Reader && !self.reader_tools.needle.is_empty();
        if reader_search {
            self.reader_tools.needle.clear();
            self.reader_tools.hits.clear();
            self.status = "Reader search cleared".into();
        }
        if self.selection.take().is_some() {
            return Ok(());
        }
        if reader_search {
            return Ok(());
        }
        if self.focus == Focus::Library && !self.library_query().is_empty() {
            self.clear_library_search();
            return Ok(());
        }
        if self.workspace.fullscreen {
            self.perform(WorkspaceCommand::Fullscreen)?;
            return Ok(());
        }
        self.clear_search()
    }

    fn clear_search(&mut self) -> Result<()> {
        if self.focus == Focus::Reader {
            if self.workspace.zen || !self.workspace.show_articles {
                self.focus_pane(Focus::Articles)?;
            } else {
                self.focus = Focus::Articles;
            }
        }
        if self.search.is_empty() {
            return Ok(());
        }
        self.retained_article = None;
        self.search.clear();
        self.page_offset = 0;
        self.focus = Focus::Articles;
        self.filter()
    }

    fn control_key(&mut self, code: KeyCode) -> Result<Action> {
        if self.reader_vim_control(code)? {
            return Ok(Action::None);
        }
        match code {
            KeyCode::Char('p') => self.open_picker(PickerKind::Commands)?,
            KeyCode::Char('d') => {
                self.move_selection(true, self.reader_height / 2)?
            }
            KeyCode::Char('u') => {
                self.move_selection(false, self.reader_height / 2)?
            }
            KeyCode::Char('f') => {
                self.move_selection(true, self.reader_height)?
            }
            KeyCode::Char('b') => {
                self.move_selection(false, self.reader_height)?
            }
            _ => {}
        }
        Ok(Action::None)
    }

    fn help_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q' | '?') | KeyCode::Esc => self.mode = Mode::Normal,
            KeyCode::Char('j') | KeyCode::Down => {
                self.help_scroll = self
                    .help_scroll
                    .saturating_add(1)
                    .min(super::HELP.lines().count().saturating_sub(1))
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.help_scroll = self.help_scroll.saturating_sub(1)
            }
            KeyCode::PageDown => {
                self.help_scroll = self
                    .help_scroll
                    .saturating_add(10)
                    .min(super::HELP.lines().count().saturating_sub(1))
            }
            KeyCode::PageUp => {
                self.help_scroll = self.help_scroll.saturating_sub(10)
            }
            KeyCode::Char('g') | KeyCode::Home => self.help_scroll = 0,
            KeyCode::Char('G') | KeyCode::End => {
                self.help_scroll = super::HELP.lines().count().saturating_sub(1)
            }
            _ => {}
        }
    }
}

fn preview_motion(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Char(
            'h' | 'j'
                | 'k'
                | 'l'
                | 'g'
                | 'G'
                | 'w'
                | 'W'
                | 'b'
                | 'B'
                | 'e'
                | 'E'
                | 'I'
                | 'A'
                | '0'..='9' | '^' | '$' | '%'
        ) | KeyCode::Up
            | KeyCode::Down
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::Backspace
            | KeyCode::PageUp
            | KeyCode::PageDown
    )
}
