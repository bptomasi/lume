//! Pane commands and keyboard layout gestures share persisted preferences.

use anyhow::{Context, Result, bail};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::layout::Rect;

use crate::{
    model::{Dock, Pane},
    ui,
};

use super::{Action, App};

impl App {
    /// Update terminal geometry before dispatching pointer events or drawing.
    pub fn resize_workspace(&mut self, area: Rect) {
        if self.screen != area {
            self.cancel_pointer();
            self.screen = area;
        }
        let areas = ui::workspace_areas(area, self.focus, &self.workspace);
        if let Some(reader) = areas.reader {
            let content = ui::workspace_reader_content(reader, &self.workspace);
            self.resize_reader(content.width.into(), content.height.into());
        }
    }

    pub(super) fn toggle_pane(&mut self, pane: Pane) -> Result<()> {
        let visible = !self.workspace.pane_visible(pane);
        if visible && pane == Pane::Annotations {
            self.select_first_annotation();
        }
        self.workspace.set_pane_visible(pane, visible);
        if visible {
            self.focus = pane;
        }
        self.commit_panel_layout()
    }

    pub(super) fn focus_pane(&mut self, pane: Pane) -> Result<()> {
        if pane == Pane::Annotations && !self.workspace.show_annotations {
            self.select_first_annotation();
        }
        self.workspace.set_pane_visible(pane, true);
        self.focus = pane;
        self.commit_panel_layout()
    }

    pub(super) fn commit_panel_layout(&mut self) -> Result<()> {
        self.workspace.fullscreen = false;
        self.workspace.zen = false;
        self.ensure_visible_focus();
        self.resize_workspace(self.screen);
        self.save_session()?;
        self.status =
            "Layout saved · Ctrl-w: focus, resize, move or collapse".into();
        Ok(())
    }

    pub(super) fn move_pane_to(&mut self, pane: Pane, target: Pane) {
        self.workspace.panel_order =
            reordered(self.workspace.panel_order, pane, target);
    }

    fn move_focused_pane(&mut self, forward: bool) -> Result<()> {
        if self.workspace.layout.is_some() {
            return self.move_spatially(if forward {
                Dock::Right
            } else {
                Dock::Left
            });
        }
        let expanded = self.move_candidates();
        let position = expanded
            .iter()
            .position(|pane| *pane == self.focus)
            .context("Expand a pane first")?;
        let target = if forward {
            position.saturating_add(1)
        } else {
            position.saturating_sub(1)
        };
        if target == position || target >= expanded.len() {
            self.status = format!(
                "{} is already at the {} edge · expand another pane to move",
                self.focus.title(),
                if forward { "right" } else { "left" }
            );
            return Ok(());
        }
        self.move_pane_to(self.focus, expanded[target]);
        self.commit_panel_layout()?;
        self.status = format!(
            "{} moved {} · layout saved",
            self.focus.title(),
            if forward { "right" } else { "left" }
        );
        Ok(())
    }

    fn move_candidates(&self) -> Vec<Pane> {
        let visible =
            ui::workspace_areas(self.screen, self.focus, &self.workspace)
                .ordered()
                .into_iter()
                .map(|(pane, _)| pane)
                .collect::<Vec<_>>();
        if visible.len() > 1 {
            return visible;
        }
        self.expanded_panes()
    }

    pub(super) fn expanded_panes(&self) -> Vec<Pane> {
        self.workspace
            .panel_order
            .into_iter()
            .filter(|pane| self.workspace.pane_visible(*pane))
            .collect()
    }

    fn resize_focused_pane(&mut self, delta: i32) -> Result<()> {
        if self.workspace.layout.is_some() {
            return self.resize_visible_split(delta);
        }
        if self.focus != Pane::Reader {
            self.workspace.resize_pane(self.focus, delta);
        } else if let Some(neighbor) = self
            .expanded_panes()
            .into_iter()
            .rev()
            .find(|pane| *pane != Pane::Reader)
        {
            self.workspace.resize_pane(neighbor, -delta);
        }
        self.commit_panel_layout()
    }

    pub(super) fn panel_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "panel" => self.toggle_pane(pane_named(argument)?)?,
            "panel-focus" => self.focus_pane(pane_named(argument)?)?,
            "panel-close" => {
                self.workspace.set_pane_visible(self.focus, false);
                self.commit_panel_layout()?;
            }
            "panel-move" => match argument {
                "left" => self.move_focused_pane(false)?,
                "right" => self.move_focused_pane(true)?,
                "up" | "above" => self.move_spatially(Dock::Above)?,
                "down" | "below" => self.move_spatially(Dock::Below)?,
                _ => bail!("Use :panel-move left|right|up|down"),
            },
            "panel-split" => self.split_focused(
                Dock::named(argument)
                    .context("Use :panel-split right|below")?,
            )?,
            "panel-resize" => {
                self.resize_focused_pane(argument.parse::<i16>()?.into())?
            }
            "annotations" => self.toggle_pane(Pane::Annotations)?,
            "reader" => self.toggle_pane(Pane::Reader)?,
            "mouse" => self.mouse_command(argument)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn mouse_command(&mut self, argument: &str) -> Result<()> {
        match argument {
            "on" => self.workspace.mouse = true,
            "off" => self.workspace.mouse = false,
            "" | "status" => {}
            _ => bail!("Use :mouse on|off|status"),
        }
        self.cancel_pointer();
        self.save_session()?;
        self.status = format!(
            "Mouse {} · {} · keyboard always available",
            if self.workspace.mouse { "on" } else { "off" },
            if self.pointer.seen {
                "terminal events received"
            } else {
                "awaiting terminal events"
            }
        );
        Ok(())
    }

    pub(super) fn panel_layout_key(&mut self, key: KeyEvent) -> Result<bool> {
        if key.modifiers == KeyModifiers::ALT
            && matches!(key.code, KeyCode::Left | KeyCode::Right)
        {
            if key.kind != KeyEventKind::Repeat {
                self.move_focused_pane(key.code == KeyCode::Right)?;
            }
            return Ok(true);
        }
        if key.code == KeyCode::Char('w')
            && key.modifiers.contains(KeyModifiers::CONTROL)
        {
            self.vim.clear_input();
            self.panel_prefix = true;
            self.status = concat!(
                "Layout: h/l focus · H/L or arrows move · ",
                "+/- resize · c collapse · a notes"
            )
            .into();
            return Ok(true);
        }
        if !self.panel_prefix {
            return Ok(false);
        }
        if key.kind == KeyEventKind::Repeat {
            return Ok(true);
        }
        self.panel_prefix = false;
        let code = match key.code {
            KeyCode::Char(character)
                if key.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                KeyCode::Char(character.to_ascii_uppercase())
            }
            code => code,
        };
        self.panel_prefix_action(code)?;
        Ok(true)
    }

    fn panel_prefix_action(&mut self, code: KeyCode) -> Result<()> {
        if self.split_layout_key(code)? {
            return Ok(());
        }
        match code {
            KeyCode::Char('h' | 'k') => self.change_focus(false),
            KeyCode::Char('l' | 'j' | 'w') => self.change_focus(true),
            KeyCode::Char('H') | KeyCode::Left => {
                self.move_focused_pane(false)?;
            }
            KeyCode::Char('L') | KeyCode::Right => {
                self.move_focused_pane(true)?;
            }
            KeyCode::Char('>' | '+') => self.resize_focused_pane(2)?,
            KeyCode::Char('<' | '-') => self.resize_focused_pane(-2)?,
            KeyCode::Char('c') => {
                self.workspace.set_pane_visible(self.focus, false);
                self.commit_panel_layout()?;
            }
            KeyCode::Char('a') => self.focus_pane(Pane::Annotations)?,
            KeyCode::Esc => {}
            _ => {
                self.status = concat!(
                    "Ctrl-w: h/l focus · H/L or arrows move · ",
                    "+/- resize · c collapse · a notes"
                )
                .into();
            }
        }
        Ok(())
    }
    fn split_layout_key(&mut self, code: KeyCode) -> Result<bool> {
        if self.workspace.layout.is_some() {
            let edge = match code {
                KeyCode::Char('h') => Some(Dock::Left),
                KeyCode::Char('j') => Some(Dock::Below),
                KeyCode::Char('k') => Some(Dock::Above),
                KeyCode::Char('l') => Some(Dock::Right),
                _ => None,
            };
            if let Some(edge) = edge {
                self.focus_spatially(edge);
                return Ok(true);
            }
        }
        match code {
            KeyCode::Char('J') | KeyCode::Down => {
                self.move_spatially(Dock::Below)?
            }
            KeyCode::Char('K') | KeyCode::Up => {
                self.move_spatially(Dock::Above)?
            }
            KeyCode::Char('s') => self.split_focused(Dock::Below)?,
            KeyCode::Char('v') => self.split_focused(Dock::Right)?,
            KeyCode::Char('u') => {
                self.layout_commands("layout-undo", "")?;
            }
            KeyCode::Char('r') => {
                self.layout_commands("layout-redo", "")?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Whether Ctrl-w is awaiting a discoverable layout operation.
    pub fn layout_prefix_pending(&self) -> bool {
        self.panel_prefix
    }
}

fn pane_named(name: &str) -> Result<Pane> {
    Pane::named(name)
        .context("Pane names: library, articles, reader, annotations")
}

/// Compute the same insertion order for previews and durable layout commits.
pub(super) fn reordered(
    mut order: [Pane; 4],
    pane: Pane,
    target: Pane,
) -> [Pane; 4] {
    let Some(from) = order.iter().position(|item| *item == pane) else {
        return order;
    };
    let Some(to) = order.iter().position(|item| *item == target) else {
        return order;
    };
    if from < to {
        order[from..=to].rotate_left(1);
    }
    if from > to {
        order[to..=from].rotate_right(1);
    }
    order
}
