//! Precise readable selections, source quotation capture and durable marks.
//!
//! Readable offsets survive soft wrapping. Stored quotes retain original
//! Markdown bytes and their source anchor. Relocation requires a unique quote
//! when the original anchor is invalid; ambiguous matches stay in the notebook.

use std::ops::Range;

use anyhow::{Context, Result, ensure};
use ratatui::style::{Color, Style};

use crate::model::Passage;
use crate::reader::{self, Rendered, cursor};

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
            KeyCode::Char('s') if self.selection.is_some() => {
                self.save_selection()?;
            }
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    pub(super) fn cursor_line(&self) -> &str {
        self.reader_lines
            .get(self.reader_cursor)
            .map_or("", String::as_str)
    }

    /// Current cursor position in readable text, independent of wrapping.
    pub fn cursor_offset(&self) -> usize {
        self.reader_ranges
            .get(self.reader_cursor)
            .map_or(0, |range| {
                range.start + self.reader_column.min(range.len())
            })
    }

    pub(super) fn clamp_cursor(&mut self) {
        self.reader_cursor = self
            .reader_cursor
            .min(self.reader_lines.len().saturating_sub(1));
        let line = self.cursor_line();
        let last = cursor::previous(line, line.len());
        self.reader_column =
            cursor::boundary(line, self.reader_column.min(last));
    }

    pub(super) fn place_cursor(&mut self, offset: usize) {
        let index = self
            .reader_ranges
            .partition_point(|range| range.start <= offset)
            .saturating_sub(1);
        self.reader_cursor =
            if self.reader_ranges.get(index).is_some_and(|range| {
                range.contains(&offset)
                    || range.is_empty() && range.start == offset
            }) {
                index
            } else {
                self.reader_ranges.len().saturating_sub(1)
            };
        self.reader_column = self
            .reader_ranges
            .get(self.reader_cursor)
            .map_or(0, |range| offset.saturating_sub(range.start));
        self.clamp_cursor();
    }

    pub(super) fn ensure_cursor_visible(&mut self) {
        if self.reader_cursor < self.scroll {
            self.scroll = self.reader_cursor;
        }
        if self.reader_cursor >= self.scroll + self.reader_height {
            self.scroll = self.reader_cursor + 1 - self.reader_height;
        }
        if self.code_lines.get(self.reader_cursor) == Some(&true) {
            let column = cursor::column(self.cursor_line(), self.reader_column);
            if column < self.horizontal {
                self.horizontal = column;
            }
            if column >= self.horizontal + self.reader_width {
                self.horizontal = column + 1 - self.reader_width;
            }
        }
        self.clamp_scroll();
    }

    pub(super) fn line_edge(&mut self, end: bool) {
        self.reader_goal = None;
        self.reader_column = if end {
            cursor::previous(self.cursor_line(), self.cursor_line().len())
        } else {
            0
        };
        self.ensure_cursor_visible();
    }

    pub(super) fn visual_selection(&mut self, linewise: bool) {
        self.pending_g = false;
        self.focus = Focus::Reader;
        self.selection =
            if self.selection.is_some() && self.linewise == linewise {
                None
            } else {
                Some(self.cursor_offset())
            };
        self.linewise = linewise;
        self.status =
            "VISUAL · hjkl / w b · y copy · s highlight · n note · Esc cancel"
                .into();
    }

    /// Inclusive visual selection as a half-open, grapheme-safe byte range.
    pub fn selected_range(&self) -> Option<Range<usize>> {
        let anchor = self.selection?;
        if anchor >= self.reader_text.len() {
            return None;
        }
        let offset = self.cursor_offset();
        let mut range = anchor.min(offset)
            ..cursor::next(&self.reader_text, anchor.max(offset));
        if self.linewise {
            let first = display_range(&self.reader_ranges, range.start);
            let last = display_range(&self.reader_ranges, range.end - 1);
            range.start = first.map_or(range.start, |r| r.start);
            range.end = last.map_or(range.end, |r| r.end);
        }
        Some(range)
    }

    pub(super) fn yank_selection(&self) -> Result<Action> {
        let range = self
            .selected_range()
            .context("Press v or V to select text")?;
        let text = self
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
        ensure!(
            visible.end <= self.article_text_end,
            "Select article text to capture a highlight"
        );
        let source = reader::to_source(&self.reader_mappings, visible)
            .context("Select article text to capture a highlight")?;
        ensure!(source.len() <= 8192, "Select a passage within 8 KiB");
        let quote = self
            .body
            .get(source.clone())
            .context("Source changed; select the passage again")?
            .to_owned();
        Ok((
            article,
            Passage {
                quote,
                anchor: source.start,
                source: self.content_source,
                revision: self.content_revision,
            },
        ))
    }

    pub(super) fn annotate_selection(&mut self) -> Result<()> {
        ensure!(
            self.notebook.is_none() && self.focus == Focus::Reader,
            "Focus Reader before creating an article annotation"
        );
        let captured = if self.selection.is_some() {
            self.captured_passage()?
        } else if self.focus == Focus::Reader
            && self.cursor_offset() < self.article_text_end
            && !self.article_preparing()
        {
            let article = self.current().context("Select an article")?.id;
            let range = self
                .reader_ranges
                .get(self.reader_cursor)
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
        self.selection = None;
        Ok(())
    }

    pub(super) fn save_selection(&mut self) -> Result<()> {
        let (article, passage) = self.captured_passage()?;
        let cursor = self.cursor_offset();
        if let Some(writer) = &mut self.note_tasks {
            writer.submit(super::note_tasks::Task::Write(
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
            self.selection = None;
            self.status =
                "Saving highlight… · navigation remains available".into();
            return Ok(());
        }
        self.store.save_passage(article, &passage, "")?;
        self.selection = None;
        self.refresh_notes(article)?;
        self.pending_anchor = None;
        self.place_cursor(cursor);
        self.ensure_cursor_visible();
        self.comment_selected = self.comments.len().saturating_sub(1);
        self.status =
            "Highlight saved · :edit-note adds a note · u undo".into();
        Ok(())
    }

    pub(super) fn paint_highlights(&self, rendered: &mut Rendered) {
        let marks = self
            .note_marks
            .passages
            .iter()
            .map(|(range, _)| range.clone())
            .collect::<Vec<_>>();
        let marks = normalize_marks(marks);
        let mut remaining = marks.as_slice();
        let style = Style::new().bg(Color::Rgb(65, 53, 32));
        for (line, row) in rendered.lines.iter_mut().zip(&rendered.text_ranges)
        {
            remaining = &remaining
                [remaining.partition_point(|mark| mark.end <= row.start)..];
            for mark in remaining.iter().take_while(|mark| mark.start < row.end)
            {
                let start = row.start.max(mark.start);
                let end = row.end.min(mark.end);
                if start < end {
                    reader::style_range(
                        line,
                        start - row.start..end - row.start,
                        style,
                    );
                }
            }
        }
    }
}

fn display_range(
    ranges: &[Range<usize>],
    offset: usize,
) -> Option<&Range<usize>> {
    let index = ranges.partition_point(|range| range.end <= offset);
    ranges.get(index).filter(|range| range.contains(&offset))
}

fn normalize_marks(mut marks: Vec<Range<usize>>) -> Vec<Range<usize>> {
    marks.sort_by_key(|range| range.start);
    let mut merged: Vec<Range<usize>> = Vec::new();
    for range in marks {
        if let Some(previous) = merged.last_mut()
            && range.start <= previous.end
        {
            previous.end = previous.end.max(range.end);
        } else {
            merged.push(range);
        }
    }
    merged
}

pub(super) fn locate_quote(
    body: &str,
    quote: &str,
    anchor: usize,
) -> Option<Range<usize>> {
    if quote.is_empty() {
        return None;
    }
    if body
        .get(anchor..)
        .is_some_and(|tail| tail.starts_with(quote))
    {
        return Some(anchor..anchor + quote.len());
    }
    let mut matches = body.match_indices(quote);
    let (start, _) = matches.next()?;
    if matches.next().is_some() {
        return None;
    }
    Some(start..start + quote.len())
}
