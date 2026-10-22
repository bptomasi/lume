//! Modal mouse input never falls through into the obscured workspace.

use anyhow::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use crate::ui;

use super::super::{Action, App, InputKind, Mode, PickerKind};

impl App {
    pub(super) fn modal_pointer(
        &mut self,
        event: MouseEvent,
    ) -> Result<Action> {
        self.cancel_pointer();
        match event.kind {
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.scroll_modal(event, 1)?;
            }
            MouseEventKind::Down(MouseButton::Left) => {
                if let Some(action) = self.click_search_result(event)? {
                    return Ok(action);
                }
                return self.click_picker(event);
            }
            _ => {}
        }
        Ok(Action::None)
    }

    fn search_pane(&self) -> Option<crate::model::Pane> {
        let Mode::Input(input) = &self.mode else {
            return None;
        };
        match input.kind {
            InputKind::LibrarySearch => Some(crate::model::Pane::Library),
            InputKind::Search => Some(crate::model::Pane::Articles),
            _ => None,
        }
    }

    fn click_search_result(
        &mut self,
        event: MouseEvent,
    ) -> Result<Option<Action>> {
        let Some(pane) = self.search_pane() else {
            return Ok(None);
        };
        let Some(area) =
            ui::workspace_areas(self.screen, self.focus, &self.workspace)
                .get(pane)
        else {
            return Ok(None);
        };
        if !area.contains(Position::new(event.column, event.row)) {
            return Ok(None);
        }
        if pane == crate::model::Pane::Library {
            let Some(selected) = ui::navigation_at(self, area, event.row)
            else {
                return Ok(None);
            };
            self.nav_selected = selected;
        } else {
            let Some(selected) = ui::article_at(self, area, event.row) else {
                return Ok(None);
            };
            self.selected = selected;
            self.load_current_deferred()?;
        }
        self.submit_input().map(Some)
    }

    fn scroll_pane_search(
        &mut self,
        event: MouseEvent,
        amount: usize,
    ) -> Result<()> {
        let Some(pane) = self.search_pane() else {
            return Ok(());
        };
        let area =
            ui::workspace_areas(self.screen, self.focus, &self.workspace)
                .get(pane);
        if area.is_some_and(|area| {
            area.contains(Position::new(event.column, event.row))
        }) {
            let forward = event.kind == MouseEventKind::ScrollDown;
            if pane == crate::model::Pane::Library {
                if let Some(selected) = self.move_library_match(forward, amount)
                {
                    self.nav_selected = selected;
                }
            } else {
                self.move_selection(forward, amount)?;
            }
        }
        Ok(())
    }

    fn click_picker(&mut self, event: MouseEvent) -> Result<Action> {
        let Mode::Picker(picker) = &mut self.mode else {
            return Ok(Action::None);
        };
        let (area, _) = ui::picker_regions(self.screen);
        if !area.contains(Position::new(event.column, event.row)) {
            return Ok(Action::None);
        }
        let first = picker.selected.saturating_sub(area.height as usize / 2);
        let selected = first + usize::from(event.row - area.y);
        if selected >= picker.choices.len() {
            return Ok(Action::None);
        }
        picker.selected = selected;
        let key = if matches!(picker.kind, PickerKind::Lists { .. }) {
            KeyCode::Char(' ')
        } else {
            KeyCode::Enter
        };
        self.picker_key(KeyEvent::new(key, KeyModifiers::NONE))
    }

    pub(super) fn scroll_modal(
        &mut self,
        event: MouseEvent,
        amount: usize,
    ) -> Result<()> {
        let forward = event.kind == MouseEventKind::ScrollDown;
        match &mut self.mode {
            Mode::Web => self.web_scroll(forward, amount),
            Mode::Image(_) => self.pan_image(forward, amount),
            Mode::Help => {
                self.help_scroll = shifted(
                    self.help_scroll,
                    super::super::HELP.lines().count(),
                    forward,
                    amount,
                )
            }
            Mode::Document(document) => {
                document.row = shifted(
                    document.row,
                    document.lines.len(),
                    forward,
                    amount,
                );
            }
            Mode::Picker(picker) => {
                picker.selected = shifted(
                    picker.selected,
                    picker.choices.len(),
                    forward,
                    amount,
                );
                self.update_picker()?;
            }
            Mode::Settings(draft) if draft.editor.is_none() => {
                draft.field = shifted(
                    draft.field,
                    super::super::SettingsDraft::FIELD_COUNT,
                    forward,
                    amount,
                )
            }
            _ => self.scroll_pane_search(event, amount)?,
        }
        Ok(())
    }
}

fn shifted(
    current: usize,
    length: usize,
    forward: bool,
    amount: usize,
) -> usize {
    if forward {
        current.saturating_add(amount).min(length.saturating_sub(1))
    } else {
        current.saturating_sub(amount)
    }
}
