//! Sequential wrapped rows consume only intersecting style and source spans.
//!
//! CommonMark appends nonoverlapping mappings in display-byte order, and textwrap
//! emits monotonically increasing UTF-8 slices. Retiring completed prefixes keeps
//! dense paragraphs from rescanning every token for each terminal row.

use std::ops::Range;

use ratatui::text::{Line, Span};

use super::{SourceMapping, to_source};

pub(super) struct Paragraph<'a> {
    spans: &'a [Span<'static>],
    mappings: &'a [SourceMapping],
    offset: usize,
}

impl<'a> Paragraph<'a> {
    pub(super) fn new(
        spans: &'a [Span<'static>],
        mappings: &'a [SourceMapping],
    ) -> Self {
        Self {
            spans,
            mappings,
            offset: 0,
        }
    }

    pub(super) fn row(
        &mut self,
        range: Range<usize>,
    ) -> (Line<'static>, Option<Range<usize>>) {
        while let Some(span) = self.spans.first()
            && self.offset + span.content.len() <= range.start
        {
            self.offset += span.content.len();
            self.spans = &self.spans[1..];
        }
        let mut offset = self.offset;
        let mut spans = Vec::new();
        for span in self.spans {
            if offset >= range.end {
                break;
            }
            let end = offset + span.content.len();
            let start_byte = range.start.max(offset);
            let end_byte = range.end.min(end);
            if start_byte < end_byte {
                spans.push(Span::styled(
                    span.content[start_byte - offset..end_byte - offset]
                        .to_owned(),
                    span.style,
                ));
            }
            offset = end;
        }
        let first = self
            .mappings
            .partition_point(|mapping| mapping.display.end <= range.start);
        self.mappings = &self.mappings[first..];
        let end = self
            .mappings
            .partition_point(|mapping| mapping.display.start < range.end);
        let source = to_source(&self.mappings[..end], range);
        (Line::from(spans), source)
    }
}

#[cfg(test)]
mod tests;
