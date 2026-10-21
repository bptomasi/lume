//! Parser-owned cells become bounded, source-mapped terminal grids.
//!
//! Table rows remain unwrapped so columns, search phrases and cursor offsets
//! stay stable across resizes. The reader's indexed horizontal viewport clips
//! wide grids. Padding has a byte budget; pathological tables retain raw source.

mod layout;

use std::ops::Range;

use pulldown_cmark::{Alignment, Event, Tag, TagEnd};
use ratatui::text::{Line, Span};

use super::{Builder, SourceMapping};

pub(super) const MAX_BYTES: usize = 1024 * 1024;

pub(super) struct Cell {
    text: String,
    spans: Vec<Span<'static>>,
    mappings: Vec<SourceMapping>,
}

pub(super) struct Pending {
    source: Range<usize>,
    alignments: Vec<Alignment>,
    rows: Vec<Vec<Cell>>,
    row: Vec<Cell>,
    bytes: usize,
    limited: bool,
}

impl Pending {
    fn new(source: Range<usize>, alignments: Vec<Alignment>) -> Self {
        let limited = alignments.len() > 64;
        Self {
            source,
            alignments,
            rows: Vec::new(),
            row: Vec::new(),
            bytes: 0,
            limited,
        }
    }

    fn cell(
        &mut self,
        mut spans: Vec<Span<'static>>,
        maps: Vec<SourceMapping>,
    ) {
        if self.limited {
            return;
        }
        for span in &mut spans {
            // ASCII controls occupy one byte each; replacing tabs/newlines
            // keeps source offsets while making cell-width measurement exact.
            span.content = span.content.replace(['\t', '\n', '\r'], " ").into();
        }
        let text = spans.iter().map(|span| span.content.as_ref()).collect();
        let cell = Cell {
            text,
            spans,
            mappings: maps,
        };
        self.bytes += cell.text.len();
        self.limited = self.bytes > MAX_BYTES || self.row.len() == 64;
        if !self.limited {
            self.row.push(cell);
        }
    }

    fn end_row(&mut self) {
        if self.rows.len() == 2000 {
            self.limited = true;
        }
        if !self.limited {
            self.rows.push(std::mem::take(&mut self.row));
        }
    }
}

impl Builder<'_> {
    pub(super) fn table_event(
        &mut self,
        event: &Event<'_>,
        range: &Range<usize>,
    ) -> bool {
        match event {
            Event::Start(Tag::Table(alignments)) => {
                self.flush();
                self.table =
                    Some(Pending::new(range.clone(), alignments.clone()));
            }
            Event::End(TagEnd::Table) => self.finish_table(),
            Event::End(TagEnd::TableCell) => self.finish_table_cell(),
            Event::End(TagEnd::TableHead | TagEnd::TableRow) => {
                if let Some(table) = &mut self.table {
                    table.end_row();
                }
            }
            Event::Start(Tag::Image { .. }) | Event::End(TagEnd::Image)
                if self.table.is_some() => {}
            Event::Html(html) | Event::InlineHtml(html)
                if self.table.is_some() =>
            {
                if html.trim().starts_with("<br") {
                    self.append(" ".into(), range.clone());
                }
            }
            Event::HardBreak if self.table.is_some() => {
                self.append(" ".into(), range.clone());
            }
            _ => return false,
        }
        true
    }

    fn finish_table_cell(&mut self) {
        if let Some(table) = &mut self.table {
            table.cell(
                std::mem::take(&mut self.spans),
                std::mem::take(&mut self.mappings),
            );
            self.text_length = 0;
        }
    }

    fn finish_table(&mut self) {
        let Some(table) = self.table.take() else {
            return;
        };
        let source = table.source.clone();
        let first = self.rendered.lines.len();
        if let Some((rows, bytes)) =
            layout::render(table, MAX_BYTES.saturating_sub(self.table_bytes))
        {
            self.table_bytes += bytes;
            for row in rows {
                self.table_row(row, source.start);
            }
            self.rendered
                .table_rows
                .push(first..self.rendered.lines.len());
        } else {
            self.generated(
                Line::raw(
                    "[Table exceeds layout limits; original Markdown follows.]",
                ),
                source.start..source.start,
            );
            if let Some(raw) = self.source.get(source.clone()) {
                self.append(raw.to_owned(), source.clone());
                self.flush();
            }
        }
        self.generated(Line::raw(""), source.end..source.end);
    }

    fn table_row(&mut self, mut row: layout::Row, anchor: usize) {
        let text = row.line.to_string();
        let source = super::to_source(&row.mappings, 0..text.len())
            .unwrap_or(anchor..anchor);
        let start = self.rendered.text.len();
        self.rendered.text.push_str(&text);
        let end = self.rendered.text.len();
        self.rendered.text.push('\n');
        for mapping in &mut row.mappings {
            mapping.display.start += start;
            mapping.display.end += start;
        }
        self.rendered.mappings.extend(row.mappings);
        self.push(row.line, source, true, start..end);
    }
}
