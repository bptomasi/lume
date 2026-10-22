//! Commands compose the same docking model used by pointer gestures.

use anyhow::{Context, Result, bail, ensure};

use crate::model::{Dock, Pane, PaneLayout, PanelLayout, Workspace};

use super::{Action, App};

impl App {
    pub(super) fn layout_commands(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "workspace" => self.profile_command(argument)?,
            "layout-undo" => self.restore_layout(false)?,
            "layout-redo" => self.restore_layout(true)?,
            "layout-reset" => self.reset_layout()?,
            "panel-dock" => self.dock_command(argument)?,
            "layout-columns" => {
                self.workspace.layout = None;
                self.commit_panel_layout()?;
            }
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn reset_layout(&mut self) -> Result<()> {
        self.cancel_pointer();
        let standard = Workspace {
            reading_width: self.workspace.reading_width,
            fullscreen_width: self.workspace.fullscreen_width,
            ..Workspace::default()
        };
        let layout = PanelLayout::capture(&standard);
        if layout != PanelLayout::capture(&self.workspace) {
            layout.apply(&mut self.workspace);
            self.fullscreen_focus = None;
            self.commit_panel_layout()?;
        }
        self.status = "Standard layout restored · Ctrl-w u undoes".into();
        Ok(())
    }

    fn profile_command(&mut self, argument: &str) -> Result<()> {
        let (verb, name) = argument.split_once(' ').unwrap_or((argument, ""));
        let name = name.trim();
        if verb == "list" || verb.is_empty() {
            let body = self
                .layouts
                .profiles
                .keys()
                .cloned()
                .collect::<Vec<_>>()
                .join("\n");
            self.show_document(
                "Workspaces · :workspace load NAME",
                &body,
                None,
            );
            return Ok(());
        }
        ensure!(
            crate::store::valid_workspace_name(name),
            "Use a workspace name of 1–64 plain characters"
        );
        self.change_profile(verb, name)?;
        self.status = format!("Workspace {verb}: {name}");
        Ok(())
    }

    fn change_profile(&mut self, verb: &str, name: &str) -> Result<()> {
        match verb {
            "save" => {
                ensure!(
                    self.layouts.profiles.contains_key(name)
                        || self.layouts.profiles.len() < 32,
                    "At most 32 workspaces"
                );
                self.layouts.profiles.insert(
                    name.into(),
                    crate::model::PanelLayout::capture(&self.workspace),
                );
                self.save_session()?;
            }
            "load" => {
                self.layouts
                    .profiles
                    .get(name)
                    .context("Workspace not found")?
                    .clone()
                    .apply(&mut self.workspace);
                self.ensure_visible_focus();
                self.resize_workspace(self.screen);
                self.save_session()?;
            }
            "delete" => {
                ensure!(
                    self.layouts.profiles.remove(name).is_some(),
                    "Workspace not found"
                );
                self.save_session()?;
            }
            _ => bail!("Use :workspace list|save NAME|load NAME|delete NAME"),
        }
        Ok(())
    }

    fn restore_layout(&mut self, redo: bool) -> Result<()> {
        let layout = if redo {
            self.layouts.redo()
        } else {
            self.layouts.undo()
        };
        let Some(layout) = layout else {
            self.status = "No layout changes in that direction".into();
            return Ok(());
        };
        self.cancel_pointer();
        layout.apply(&mut self.workspace);
        self.ensure_visible_focus();
        self.resize_workspace(self.screen);
        self.save_session()?;
        self.status = "Layout restored · :layout-undo / :layout-redo".into();
        Ok(())
    }

    fn dock_command(&mut self, argument: &str) -> Result<()> {
        let parts = argument.split_whitespace().collect::<Vec<_>>();
        ensure!(
            parts.len() == 3,
            "Use :panel-dock PANE TARGET left|right|above|below"
        );
        let pane = Pane::named(parts[0]).context("Unknown pane")?;
        let target = Pane::named(parts[1]).context("Unknown target pane")?;
        let edge =
            Dock::named(parts[2]).context("Unknown docking direction")?;
        self.dock_pane(pane, target, edge)?;
        self.focus = pane;
        self.commit_panel_layout()
    }

    pub(super) fn dock_pane(
        &mut self,
        pane: Pane,
        target: Pane,
        edge: Dock,
    ) -> Result<()> {
        let tree = self.workspace.layout.clone().unwrap_or_else(|| {
            PaneLayout::columns(&self.workspace.panel_order)
        });
        self.workspace.layout = Some(
            tree.dock(pane, target, edge)
                .context("Choose two different panes in a valid layout")?,
        );
        self.workspace.set_pane_visible(pane, true);
        self.workspace.set_pane_visible(target, true);
        Ok(())
    }
}
