//! Grapheme-safe selection and cursor invariants over the cached projection.
use super::ReaderState;
use crate::reader::{self, Rendered, cursor};
use ratatui::style::{Color, Style};
use std::ops::Range;

impl ReaderState {
    pub(super) fn cursor_line(&self) -> &str {
        self.projection
            .lines
            .get(self.viewport.row)
            .map_or("", String::as_str)
    }

    pub(in crate::app) fn cursor_offset(&self) -> usize {
        self.projection
            .ranges
            .get(self.viewport.row)
            .map_or(0, |range| {
                range.start + self.viewport.byte.min(range.len())
            })
    }

    pub(in crate::app) fn clamp_cursor(&mut self) {
        self.viewport.row = self
            .viewport
            .row
            .min(self.projection.lines.len().saturating_sub(1));
        let line = self.cursor_line();
        let last = cursor::previous(line, line.len());
        self.viewport.byte =
            cursor::boundary(line, self.viewport.byte.min(last));
    }

    pub(in crate::app) fn place_cursor(&mut self, offset: usize) {
        let index = self
            .projection
            .ranges
            .partition_point(|range| range.start <= offset)
            .saturating_sub(1);
        // Soft wrapping omits separator whitespace. Clamp such offsets to the
        // preceding visible row instead of treating the gap as end-of-document.
        self.viewport.row = index;
        self.viewport.byte = self
            .projection
            .ranges
            .get(self.viewport.row)
            .map_or(0, |range| offset.saturating_sub(range.start));
        self.clamp_cursor();
    }

    pub(in crate::app) fn ensure_cursor_visible(&mut self) {
        self.select_horizontal_block();
        if self.viewport.row < self.viewport.first {
            self.viewport.first = self.viewport.row;
        }
        if self.viewport.row >= self.viewport.first + self.viewport.height {
            self.viewport.first = self.viewport.row + 1 - self.viewport.height;
        }
        if self.projection.code.get(self.viewport.row) == Some(&true) {
            let column = self.row_column(self.viewport.row, self.viewport.byte);
            if column < self.viewport.horizontal {
                self.viewport.horizontal = column;
            }
            if column >= self.viewport.horizontal + self.viewport.width {
                self.viewport.horizontal = column + 1 - self.viewport.width;
            }
        }
        self.clamp_scroll();
    }

    pub(in crate::app) fn line_edge(&mut self, end: bool) {
        self.viewport.goal = None;
        self.viewport.byte = if end {
            cursor::previous(self.cursor_line(), self.cursor_line().len())
        } else {
            0
        };
        self.ensure_cursor_visible();
    }

    pub(in crate::app) fn selected_range(&self) -> Option<Range<usize>> {
        selected_range(
            &self.projection.text,
            &self.projection.ranges,
            self.viewport.selection,
            self.cursor_offset(),
            self.viewport.linewise,
        )
    }

    pub(super) fn paint_highlights(&self, rendered: &mut Rendered) {
        let marks = self
            .notes
            .marks
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

impl ReaderState {
    pub(in crate::app) fn capture_passage(
        &self,
        visible: Range<usize>,
    ) -> anyhow::Result<crate::model::Passage> {
        use anyhow::{Context, ensure};
        ensure!(
            visible.end <= self.projection.article_end,
            "Select article text to capture a highlight",
        );
        let source = reader::to_source(&self.projection.mappings, visible)
            .context("Select article text to capture a highlight")?;
        ensure!(source.len() <= 8192, "Select a passage within 8 KiB");
        let quote = self
            .publisher
            .body
            .get(source.clone())
            .context("Source changed; select the passage again")?
            .to_owned();
        Ok(crate::model::Passage {
            quote,
            anchor: source.start,
            source: self.publisher.source,
            revision: self.publisher.revision,
        })
    }
}

pub(super) fn selected_range(
    text: &str,
    ranges: &[Range<usize>],
    selection: Option<usize>,
    offset: usize,
    linewise: bool,
) -> Option<Range<usize>> {
    let anchor = selection?;
    if anchor >= text.len() {
        return None;
    }
    let mut range = anchor.min(offset)..cursor::next(text, anchor.max(offset));
    if linewise {
        let first = display_range(ranges, range.start);
        let last = display_range(ranges, range.end - 1);
        range.start = first.map_or(range.start, |r| r.start);
        range.end = last.map_or(range.end, |r| r.end);
    }
    Some(range)
}
