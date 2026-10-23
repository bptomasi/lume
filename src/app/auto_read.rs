//! Automatic state changes observe visible publisher text, never pointer selection.
mod state;

use super::{Action, App, Focus, Mode};
use anyhow::Result;
use std::time::Instant;

#[derive(Default)]
pub(super) struct AutoRead {
    state: state::State,
    unfocused: bool,
}

impl App {
    /// Observe active reading without disk work until one write is admitted.
    ///
    /// # Errors
    /// Returns write admission or synchronous persistence/count failures.
    pub fn poll_auto_read(&mut self, now: Instant) -> Result<bool> {
        let Some(identity) = self.reader_identity() else {
            self.auto_read.state.pause();
            return Ok(false);
        };
        self.auto_read.state.source(
            identity,
            self.article_text_end,
            self.workspace.auto_read,
        );
        if !self.auto_read_active() {
            self.auto_read.state.pause();
            return Ok(false);
        }
        self.observe_auto_read_viewport();
        if !self.auto_read.state.tick(now) {
            return Ok(false);
        }
        if let Err(error) =
            self.submit_read_state([identity.article].into(), false)
        {
            // Failed admission is reported once; explicit marking can retry.
            self.auto_read.state.block(identity.article);
            return Err(error);
        }
        self.auto_read.state.committed_intent();
        self.retained_article = Some(identity.article);
        if self.personal_writes.is_none() {
            self.refresh_read_counts()?;
        }
        self.status =
            "Marked read after active reading · m marks unread · u undo".into();
        Ok(true)
    }

    fn auto_read_active(&self) -> bool {
        self.workspace.auto_read.enabled
            && !self.auto_read.unfocused
            && self.focus == Focus::Reader
            && matches!(self.mode, Mode::Normal)
            && self.selection.is_none()
            && self.notebook.is_none()
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

    fn observe_auto_read_viewport(&mut self) {
        let view = (
            self.scroll,
            self.reader_height,
            self.reader_width,
            self.horizontal,
        );
        if self.auto_read.state.view == Some(view) {
            return;
        }
        self.auto_read.state.view = Some(view);
        let last = self
            .scroll
            .saturating_add(self.reader_height)
            .min(self.reader_ranges.len());
        for row in self.scroll..last {
            let Some(range) = self.reader_ranges.get(row) else {
                break;
            };
            if range.start >= self.article_text_end {
                break;
            }
            let next = self
                .reader_ranges
                .get(row + 1)
                .map_or(self.article_text_end, |next| next.start)
                .min(self.article_text_end);
            let mut visible = range.start..next;
            if self.code_lines.get(row) == Some(&true) {
                let skip = self.horizontal_for_row(row);
                let left = self.row_byte_at(row, skip);
                let right = self
                    .row_byte_at(row, skip.saturating_add(self.reader_width));
                visible.start += left;
                if right < range.len() {
                    visible.end = range.start + right;
                }
            }
            self.auto_read.state.record(visible);
        }
    }

    pub(super) fn auto_read_window_focus(&mut self, focused: bool) {
        self.auto_read.unfocused = !focused;
        self.auto_read.state.pause();
    }

    pub(super) fn auto_read_manual_state(&mut self, article: i64) {
        self.auto_read.state.block(article);
    }

    pub(super) fn auto_read_text_changed(&mut self) {
        self.auto_read.state.reset_text();
    }

    pub(super) fn auto_read_command(
        &mut self,
        argument: &str,
    ) -> Result<Option<Action>> {
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
                return Ok(Some(Action::None));
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
        Ok(Some(Action::None))
    }
}
