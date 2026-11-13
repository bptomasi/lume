//! Pane commands and keyboard layout gestures share persisted preferences.

use crate::app::commands::routing;

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
        self.pin_navigation_viewport();
        if self.layout.screen() != area {
            let height_changed = self.layout.screen().height != area.height;
            self.cancel_pointer();
            self.reader.arm_note_delete(None);
            self.layout.resize(area);
            if height_changed {
                self.resize_web_height();
            }
        }
        let areas = ui::workspace_areas(
            area,
            self.layout.focus(),
            self.layout.workspace(),
        );
        if let Some(articles) = areas.articles {
            self.nav.pin_articles(ui::article_origin(self, articles));
        }
        if let Some(reader) = areas.reader {
            if self.notebook().is_some() {
                self.resize_notebook_context(
                    reader.width.saturating_sub(2).into(),
                );
                return;
            }
            let content =
                ui::workspace_reader_content(reader, self.layout.workspace());
            self.resize_reader(content.width.into(), content.height.into());
        }
    }

    pub(super) fn toggle_pane(&mut self, pane: Pane) -> Result<()> {
        let visible = !self.layout.workspace().pane_visible(pane);
        if visible && pane == Pane::Annotations {
            self.select_first_annotation();
        }
        self.layout.show_pane(pane, visible);
        if visible {
            self.layout.focus_pane(pane);
        }
        self.commit_panel_layout()
    }

    pub(super) fn focus_pane(&mut self, pane: Pane) -> Result<()> {
        if pane == Pane::Annotations
            && !self.layout.workspace().show_annotations
        {
            self.select_first_annotation();
        }
        self.layout.show_pane(pane, true);
        self.layout.focus_pane(pane);
        self.commit_panel_layout()
    }

    pub(super) fn commit_panel_layout(&mut self) -> Result<()> {
        self.layout.set_fullscreen(false);
        self.layout.set_zen(false);
        self.ensure_visible_focus();
        self.resize_workspace(self.layout.screen());
        if self.layout.history().current
            != crate::model::PanelLayout::capture(self.layout.workspace())
        {
            self.save_session()?;
        }
        self.interaction.report(
            "Layout saved · Ctrl-w: focus, resize, move or collapse".into(),
        );
        Ok(())
    }

    pub(super) fn move_pane_to(&mut self, pane: Pane, target: Pane) {
        self.layout.reorder_pane(pane, target);
    }

    fn move_focused_pane(&mut self, forward: bool) -> Result<()> {
        if self.layout.workspace().layout.is_some() {
            return self.move_spatially(if forward {
                Dock::Right
            } else {
                Dock::Left
            });
        }
        let expanded = self.move_candidates();
        let position = expanded
            .iter()
            .position(|pane| *pane == self.layout.focus())
            .context("Expand a pane first")?;
        let target = if forward {
            position.saturating_add(1)
        } else {
            position.saturating_sub(1)
        };
        if target == position || target >= expanded.len() {
            self.interaction.report(format!(
                "{} is already at the {} edge · expand another pane to move",
                self.layout.focus().title(),
                if forward { "right" } else { "left" }
            ));
            return Ok(());
        }
        self.move_pane_to(self.layout.focus(), expanded[target]);
        self.commit_panel_layout()?;
        self.interaction.report(format!(
            "{} moved {} · layout saved",
            self.layout.focus().title(),
            if forward { "right" } else { "left" }
        ));
        Ok(())
    }

    fn move_candidates(&self) -> Vec<Pane> {
        let visible = ui::workspace_areas(
            self.layout.screen(),
            self.layout.focus(),
            self.layout.workspace(),
        )
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
        self.layout
            .workspace()
            .panel_order
            .into_iter()
            .filter(|pane| self.layout.workspace().pane_visible(*pane))
            .collect()
    }

    fn resize_focused_pane(&mut self, delta: i32) -> Result<()> {
        if self.layout.workspace().layout.is_some() {
            return self.resize_visible_split(delta);
        }
        if self.layout.focus() != Pane::Reader {
            self.layout.resize_pane(self.layout.focus(), delta);
        } else if let Some(neighbor) = self
            .expanded_panes()
            .into_iter()
            .rev()
            .find(|pane| *pane != Pane::Reader)
        {
            self.layout.resize_pane(neighbor, -delta);
        }
        self.commit_panel_layout()
    }

    pub(super) fn panel_command(
        &mut self,
        command: routing::PanelCommand,
        argument: &str,
    ) -> Result<Action> {
        match command {
            routing::PanelCommand::Panel => {
                self.toggle_pane(pane_named(argument)?)?
            }
            routing::PanelCommand::PanelFocus => {
                self.focus_pane(pane_named(argument)?)?
            }
            routing::PanelCommand::PanelClose => {
                self.layout.show_pane(self.layout.focus(), false);
                self.commit_panel_layout()?;
            }
            routing::PanelCommand::PanelMove => match argument {
                "left" => self.move_focused_pane(false)?,
                "right" => self.move_focused_pane(true)?,
                "up" | "above" => self.move_spatially(Dock::Above)?,
                "down" | "below" => self.move_spatially(Dock::Below)?,
                _ => bail!("Use :panel-move left|right|up|down"),
            },
            routing::PanelCommand::PanelSplit => self.split_focused(
                Dock::named(argument)
                    .context("Use :panel-split right|below")?,
            )?,
            routing::PanelCommand::PanelResize => {
                self.resize_focused_pane(argument.parse::<i16>()?.into())?
            }
            routing::PanelCommand::Annotations => {
                self.toggle_pane(Pane::Annotations)?
            }
            routing::PanelCommand::Reader => self.toggle_pane(Pane::Reader)?,
            routing::PanelCommand::Mouse => self.mouse_command(argument)?,
        }
        Ok(Action::None)
    }

    fn mouse_command(&mut self, argument: &str) -> Result<()> {
        match argument {
            "on" => self.layout.set_mouse(true),
            "off" => self.layout.set_mouse(false),
            "" | "status" => {}
            _ => bail!("Use :mouse on|off|status"),
        }
        self.cancel_pointer();
        self.save_session()?;
        self.interaction.report(format!(
            "Mouse {} · {} · keyboard always available",
            if self.layout.workspace().mouse {
                "on"
            } else {
                "off"
            },
            if self.pointer.seen {
                "terminal events received"
            } else {
                "awaiting terminal events"
            }
        ));
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
            self.interaction.vim_input().clear_input();
            self.interaction.set_panel_prefix(true);
            self.interaction.report(
                concat!(
                    "Layout: h/l focus · H/L or arrows move · ",
                    "+/- resize · c collapse · a notes · = reset"
                )
                .into(),
            );
            return Ok(true);
        }
        if !self.interaction.panel_prefix() {
            return Ok(false);
        }
        if key.kind == KeyEventKind::Repeat {
            return Ok(true);
        }
        self.interaction.set_panel_prefix(false);
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
            KeyCode::Char('w') => self.change_focus(true),
            KeyCode::Char('H') | KeyCode::Left => {
                self.move_focused_pane(false)?;
            }
            KeyCode::Char('L') | KeyCode::Right => {
                self.move_focused_pane(true)?;
            }
            KeyCode::Char('>' | '+') => self.resize_focused_pane(2)?,
            KeyCode::Char('<' | '-') => self.resize_focused_pane(-2)?,
            KeyCode::Char('c') => {
                self.layout.show_pane(self.layout.focus(), false);
                self.commit_panel_layout()?;
            }
            KeyCode::Char('a') => self.focus_pane(Pane::Annotations)?,
            KeyCode::Esc => {}
            _ => {
                self.interaction.report(
                    concat!(
                        "Ctrl-w: h/l focus · H/L or arrows move · ",
                        "+/- resize · c collapse · a notes · = reset"
                    )
                    .into(),
                );
            }
        }
        Ok(())
    }

    fn split_layout_key(&mut self, code: KeyCode) -> Result<bool> {
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
        use routing::LayoutCommand as C;

        match code {
            KeyCode::Char('=') => {
                self.layout_commands(C::LayoutReset, "")?;
            }
            KeyCode::Char('J') | KeyCode::Down => {
                self.move_spatially(Dock::Below)?
            }
            KeyCode::Char('K') | KeyCode::Up => {
                self.move_spatially(Dock::Above)?
            }
            KeyCode::Char('s') => self.split_focused(Dock::Below)?,
            KeyCode::Char('v') => self.split_focused(Dock::Right)?,
            KeyCode::Char('u') => {
                self.layout_commands(C::LayoutUndo, "")?;
            }
            KeyCode::Char('r') => {
                self.layout_commands(C::LayoutRedo, "")?;
            }
            _ => return Ok(false),
        }
        Ok(true)
    }

    /// Whether Ctrl-w is awaiting a discoverable layout operation.
    pub fn layout_prefix_pending(&self) -> bool {
        self.interaction.panel_prefix()
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
