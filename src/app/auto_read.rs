//! Automatic state changes observe visible publisher text, never pointer selection.

use super::{Action, App, Focus, Mode};
use anyhow::Result;
use std::time::Instant;

impl App {
    /// Observe active reading without disk work until one write is admitted.
    ///
    /// # Errors
    /// Returns write admission or synchronous persistence/count failures.
    pub fn poll_auto_read(&mut self, now: Instant) -> Result<bool> {
        let Some(identity) = self.reader.observe_reading(
            now,
            self.workspace.auto_read,
            self.auto_read_active(),
        ) else {
            return Ok(false);
        };
        if let Err(error) =
            self.submit_read_state([identity.article].into(), false)
        {
            // Failed admission is reported once; explicit marking can retry.
            self.reader.auto_read_manual_state(identity.article);
            return Err(error);
        }
        self.reader.auto_read_admitted();
        self.nav.retain_article(Some(identity.article));
        if self.personal_writes.is_none() {
            self.refresh_read_counts()?;
        }
        self.status =
            "Marked read after active reading · m marks unread · u undo".into();
        Ok(true)
    }

    fn auto_read_active(&self) -> bool {
        self.workspace.auto_read.enabled
            && self.focus == Focus::Reader
            && matches!(self.mode, Mode::Normal)
            && !self.reader.selecting()
            && self.notebook().is_none()
            && !self.reader_stale()
            && !self.article_preparing()
            && !self.pane_motion_active()
            && self.panel_drag().is_none()
            && self.current().is_some_and(|article| article.unread)
            && crate::ui::workspace_areas(
                self.screen,
                self.focus,
                &self.workspace,
            )
            .reader
            .is_some_and(|area| {
                !crate::ui::workspace_reader_content(area, &self.workspace)
                    .is_empty()
            })
    }

    pub(super) fn auto_read_command(
        &mut self,
        argument: &str,
    ) -> Result<Action> {
        let mut policy = self.workspace.auto_read;
        match argument.trim() {
            "on" => policy.enabled = true,
            "off" => policy.enabled = false,
            "toggle" => policy.enabled = !policy.enabled,
            "" | "status" => {
                self.status = format!(
                    "Auto read: {} · {} active seconds · {}% displayed coverage",
                    policy.enabled,
                    policy.min_active_seconds,
                    policy.min_coverage_percent
                );
                return Ok(Action::None);
            }
            _ => anyhow::bail!(
                "Use :auto-read on|off|toggle|status; thresholds are in :settings or TOML"
            ),
        }
        self.workspace.auto_read = policy;
        self.persist_preferences()?;
        self.status = format!(
            "Automatic reading {}",
            if policy.enabled {
                "enabled"
            } else {
                "disabled"
            }
        );
        Ok(Action::None)
    }

    pub(super) fn auto_read_window_focus(&mut self, focused: bool) {
        self.reader.auto_read_window_focus(focused);
    }

    pub(super) fn auto_read_manual_state(&mut self, article: i64) {
        self.reader.auto_read_manual_state(article);
    }
}
