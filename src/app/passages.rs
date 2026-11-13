//! Precise readable selections, source quotation capture and durable marks.
//!
//! Readable offsets survive soft wrapping. Stored quotes retain original
//! Markdown bytes and their source anchor. Relocation requires a unique quote
//! when the original anchor is invalid; ambiguous matches stay in the notebook.

use std::ops::Range;

use anyhow::{Context, Result, ensure};

use crate::model::Passage;

use super::{Action, App, Focus, InputKind};

impl App {
    pub(super) fn passage_key(
        &mut self,
        code: crossterm::event::KeyCode,
    ) -> Result<Option<Action>> {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Char('v') => self.visual_selection(false),
            KeyCode::Char('V') => self.visual_selection(true),
            KeyCode::Char('y') if self.focus == Focus::Reader => {
                return self.yank_selection().map(Some);
            }
            KeyCode::Char('s') if self.reader.selecting() => {
                self.save_selection()?;
            }
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    pub(super) fn visual_selection(&mut self, linewise: bool) {
        self.pending_g = false;
        self.focus = Focus::Reader;
        self.reader.toggle_selection(linewise);
        self.status =
            "VISUAL · hjkl / w b · y copy · s highlight · n note · Esc cancel"
                .into();
    }

    pub(super) fn yank_selection(&self) -> Result<Action> {
        let range = self
            .selected_range()
            .context("Press v or V to select text")?;
        let text = self
            .reader
            .view()
            .reader_text
            .get(range)
            .context("Select the text again")?;
        ensure!(!text.is_empty(), "Select text before yanking");
        ensure!(text.len() <= 65_536, "Yank selection exceeds 64 KiB");
        Ok(Action::Copy(text.to_owned()))
    }

    pub(super) fn captured_passage(&self) -> Result<(i64, Passage)> {
        let article = self.current().context("Select an article first")?.id;
        let visible = self
            .selected_range()
            .context("Press v to select a passage")?;
        self.passage_for_range(article, visible)
    }

    fn passage_for_range(
        &self,
        article: i64,
        visible: Range<usize>,
    ) -> Result<(i64, Passage)> {
        self.guard_stale_command(crate::command::Command::Annotate)?;
        Ok((article, self.reader.capture_passage(visible)?))
    }

    pub(super) fn annotate_selection(&mut self) -> Result<()> {
        ensure!(
            self.notebook.is_none() && self.focus == Focus::Reader,
            "Focus Reader before creating an article annotation"
        );
        let captured = if self.reader.selecting() {
            self.captured_passage()?
        } else if self.focus == Focus::Reader
            && self.cursor_offset() < self.reader.publisher_end()
            && !self.article_preparing()
        {
            let article = self.current().context("Select an article")?.id;
            let range = self
                .reader
                .view()
                .reader_ranges
                .get(self.reader.cursor_row())
                .context("Select a readable article row")?
                .clone();
            self.passage_for_range(article, range)?
        } else {
            return self.begin_reader_note();
        };
        let (article, passage) = captured;
        self.ensure_no_recovery(&crate::model::DraftTarget::Passage {
            article,
            passage: passage.clone(),
        })?;
        self.begin_input(
            InputKind::Highlight { article, passage },
            String::new(),
        );
        self.reader.cancel_selection();
        Ok(())
    }

    pub(super) fn save_selection(&mut self) -> Result<()> {
        let (article, passage) = self.captured_passage()?;
        let cursor = self.cursor_offset();
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(super::personal_tasks::Task::Write(
                crate::model::NoteDraft {
                    target: crate::model::DraftTarget::Passage {
                        article,
                        passage,
                    },
                    body: String::new(),
                    baseline: String::new(),
                    initial_kind: None,
                },
            ))?;
            self.reader.cancel_selection();
            self.status =
                "Saving highlight… · navigation remains available".into();
            return Ok(());
        }
        self.store.save_passage(article, &passage, "")?;
        self.reader.cancel_selection();
        self.refresh_notes(article)?;
        self.reader.cancel_restore();
        self.place_cursor(cursor);
        self.ensure_cursor_visible();
        self.reader.select_comment(
            self.reader.view().comments.len().saturating_sub(1),
        );
        self.status =
            "Highlight saved · :edit-note adds a note · u undo".into();
        Ok(())
    }
}
