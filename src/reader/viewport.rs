//! Sparse cell checkpoints bound repeated projection of unwrapped code rows.
//!
//! Build once when a styled row changes. Checkpoints retain full Unicode and
//! tab context; drawing visits only a short prefix and the visible graphemes.
//! Span ends locate cached syntax/highlight styles without copying hidden text.

use std::ops::Range;

use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_segmentation::UnicodeSegmentation;

use super::cursor;

#[derive(Clone, Copy, Default)]
struct Position {
    byte: usize,
    column: usize,
}

/// Width/byte and syntax-span index for one immutable display row.
#[derive(Clone, Default)]
pub(crate) struct RowIndex {
    points: Vec<Position>,
    span_ends: Vec<usize>,
}

impl RowIndex {
    /// Index the exact readable row and its concatenated styled spans.
    pub(crate) fn new(text: &str, line: &Line<'_>) -> Self {
        let mut points = vec![Position::default()];
        let mut column = 0;
        let mut next = 256;
        for (byte, grapheme) in text.grapheme_indices(true) {
            if byte >= next {
                points.push(Position { byte, column });
                next = byte.saturating_add(256);
            }
            column += cursor::width(grapheme, column);
        }
        let mut end = 0;
        let span_ends = line
            .spans
            .iter()
            .map(|span| {
                end += span.content.len();
                end
            })
            .collect();
        Self { points, span_ends }
    }

    fn before_column(&self, column: usize) -> Position {
        let index = self.points.partition_point(|point| point.column <= column);
        self.points
            .get(index.saturating_sub(1))
            .copied()
            .unwrap_or_default()
    }

    /// Cell column of a grapheme-aligned byte in the indexed row.
    pub(crate) fn column(&self, text: &str, byte: usize) -> usize {
        let index = self.points.partition_point(|point| point.byte <= byte);
        let point = self
            .points
            .get(index.saturating_sub(1))
            .copied()
            .unwrap_or_default();
        text.get(point.byte..byte)
            .unwrap_or_default()
            .graphemes(true)
            .fold(point.column, |column, grapheme| {
                column + cursor::width(grapheme, column)
            })
    }

    /// Grapheme at a terminal column, including columns inside tabs/wide cells.
    pub(crate) fn byte_at(&self, text: &str, goal: usize) -> usize {
        let point = self.before_column(goal);
        let mut column = point.column;
        for (byte, grapheme) in text[point.byte..].grapheme_indices(true) {
            let next = column + cursor::width(grapheme, column);
            if next > goal {
                return point.byte + byte;
            }
            column = next;
        }
        cursor::previous(text, text.len())
    }

    /// Project visible cells and decorate original byte ranges, never a prefix copy.
    pub(crate) fn project(
        &self,
        line: &Line<'_>,
        text: &str,
        skip: usize,
        width: usize,
        mut decorate: impl FnMut(Range<usize>, Style) -> Style,
    ) -> Line<'static> {
        let point = self.before_column(skip);
        let mut column = point.column;
        let end = skip.saturating_add(width);
        let mut spans = Vec::new();
        for (byte, grapheme) in text[point.byte..].grapheme_indices(true) {
            if column >= end {
                break;
            }
            let next = column + cursor::width(grapheme, column);
            if next > skip {
                let byte = point.byte + byte;
                let span = self.span_ends.partition_point(|end| *end <= byte);
                let style =
                    line.spans.get(span).map_or(line.style, |span| span.style);
                let style = decorate(byte..byte + grapheme.len(), style);
                append(&mut spans, grapheme, column..next, skip..end, style);
            }
            column = next;
        }
        Line::from(spans).style(line.style)
    }
}

fn append(
    spans: &mut Vec<Span<'static>>,
    grapheme: &str,
    cells: Range<usize>,
    viewport: Range<usize>,
    style: Style,
) {
    let text = if grapheme == "\t"
        || cells.start < viewport.start
        || cells.end > viewport.end
    {
        " ".repeat(
            cells.end.min(viewport.end) - cells.start.max(viewport.start),
        )
    } else {
        grapheme.to_owned()
    };
    if let Some(last) = spans.last_mut()
        && last.style == style
    {
        last.content.to_mut().push_str(&text);
    } else {
        spans.push(Span::styled(text, style));
    }
}

#[cfg(test)]
mod tests;
