//! Commands compose the same docking model used by pointer gestures.

use crate::app::commands::routing;

use anyhow::{Context, Result, bail, ensure};

use crate::model::{Dock, Pane, PanelLayout, Workspace};

use super::{Action, App};

impl App {
    pub(super) fn layout_commands(
        &mut self,
        command: routing::LayoutCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::LayoutCommand as C;

        match command {
            C::Workspace => self.profile_command(argument)?,
            C::LayoutUndo => self.restore_layout(false)?,
            C::LayoutRedo => self.restore_layout(true)?,
            C::LayoutReset => self.reset_layout()?,
            C::PanelDock => self.dock_command(argument)?,
            C::LayoutColumns => {
                self.layout.reset_columns();
                self.commit_panel_layout()?;
            }
        }
        Ok(Action::None)
    }

    fn reset_layout(&mut self) -> Result<()> {
        self.cancel_pointer();
        let standard = Workspace {
            reading_width: self.layout.workspace().reading_width,
            fullscreen_width: self.layout.workspace().fullscreen_width,
            ..Workspace::default()
        };
        let layout = PanelLayout::capture(&standard);
        if layout != PanelLayout::capture(self.layout.workspace()) {
            self.layout.apply_panel_layout(&layout);
            self.layout.remember_fullscreen_focus(None);
            self.commit_panel_layout()?;
        }
        self.interaction
            .report("Standard layout restored · Ctrl-w u undoes".into());
        Ok(())
    }

    fn profile_command(&mut self, argument: &str) -> Result<()> {
        let (verb, name) = argument.split_once(' ').unwrap_or((argument, ""));
        let name = name.trim();
        if verb == "list" || verb.is_empty() {
            let body = self
                .layout
                .history()
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
            crate::validation::valid_workspace_name(name),
            "Use a workspace name of 1–64 plain characters"
        );
        self.change_profile(verb, name)?;
        self.interaction.report(format!("Workspace {verb}: {name}"));
        Ok(())
    }

    fn change_profile(&mut self, verb: &str, name: &str) -> Result<()> {
        match verb {
            "save" => {
                self.layout.save_profile(name)?;
                self.save_session()?;
            }
            "load" => {
                self.layout.load_profile(name)?;
                self.ensure_visible_focus();
                self.resize_workspace(self.layout.screen());
                self.save_session()?;
            }
            "delete" => {
                self.layout.delete_profile(name)?;
                self.save_session()?;
            }
            _ => bail!("Use :workspace list|save NAME|load NAME|delete NAME"),
        }
        Ok(())
    }

    fn restore_layout(&mut self, redo: bool) -> Result<()> {
        let layout = self.layout.restore_history(redo);
        let Some(layout) = layout else {
            self.interaction
                .report("No layout changes in that direction".into());
            return Ok(());
        };
        self.cancel_pointer();
        self.layout.apply_panel_layout(&layout);
        self.ensure_visible_focus();
        self.resize_workspace(self.layout.screen());
        self.save_session()?;
        self.interaction
            .report("Layout restored · :layout-undo / :layout-redo".into());
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
        self.layout.focus_pane(pane);
        self.commit_panel_layout()
    }

    pub(super) fn dock_pane(
        &mut self,
        pane: Pane,
        target: Pane,
        edge: Dock,
    ) -> Result<()> {
        self.layout.dock(pane, target, edge)
    }
}
