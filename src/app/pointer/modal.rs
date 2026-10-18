//! Modal mouse input never falls through into the obscured workspace.

use anyhow::Result;
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use ratatui::layout::Position;

use crate::ui;

use super::super::{Action, App, Mode, PickerKind};

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
                return self.click_picker(event);
            }
            _ => {}
        }
        Ok(Action::None)
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
            Mode::Settings(draft) => {
                draft.field = shifted(
                    draft.field,
                    super::super::SettingsDraft::FIELD_COUNT,
                    forward,
                    amount,
                )
            }
            _ => {}
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
