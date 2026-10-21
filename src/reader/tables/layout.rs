//! Unicode cell widths and Markdown alignment share one grid layout owner.

use pulldown_cmark::Alignment;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
};
use unicode_width::UnicodeWidthStr;

use super::{Cell, Pending};
use crate::reader::{AMBER, SourceMapping, TEXT};

const BORDER: Color = Color::Rgb(54, 52, 51);

pub(super) struct Row {
    pub(super) line: Line<'static>,
    pub(super) mappings: Vec<SourceMapping>,
}

pub(super) fn render(
    table: Pending,
    budget: usize,
) -> Option<(Vec<Row>, usize)> {
    if table.limited || table.rows.is_empty() {
        return None;
    }
    let widths = widths(&table)?;
    let mut rows = vec![rule(&widths, ['╭', '┬', '╮'])];
    let mut bytes = rows[0].line.to_string().len() + 1;
    for (index, cells) in table.rows.into_iter().enumerate() {
        let row = data(cells, &widths, &table.alignments, index == 0);
        bytes += row.line.to_string().len() + 1;
        if bytes > budget {
            return None;
        }
        rows.push(row);
        if index == 0 {
            let header = rule(&widths, ['├', '┼', '┤']);
            bytes += header.line.to_string().len() + 1;
            rows.push(header);
        }
    }
    let bottom = rule(&widths, ['╰', '┴', '╯']);
    bytes += bottom.line.to_string().len() + 1;
    rows.push(bottom);
    (bytes <= budget).then_some((rows, bytes))
}

fn widths(table: &Pending) -> Option<Vec<usize>> {
    let mut widths = vec![1; table.alignments.len()];
    for row in &table.rows {
        for (width, cell) in widths.iter_mut().zip(row) {
            *width = (*width).max(cell.text.width());
            if *width > 4096 {
                return None;
            }
        }
    }
    let total_columns = widths.iter().sum::<usize>() + widths.len() * 3 + 1;
    (total_columns <= 4096).then_some(widths)
}

fn rule(widths: &[usize], edges: [char; 3]) -> Row {
    let mut text = edges[0].to_string();
    for (index, width) in widths.iter().enumerate() {
        if index > 0 {
            text.push(edges[1]);
        }
        text.push_str(&"─".repeat(width + 2));
    }
    text.push(edges[2]);
    Row {
        line: Line::styled(text, Style::new().fg(BORDER)),
        mappings: Vec::new(),
    }
}

fn data(
    cells: Vec<Cell>,
    widths: &[usize],
    alignments: &[Alignment],
    header: bool,
) -> Row {
    let mut row = Row {
        line: Line::default(),
        mappings: Vec::new(),
    };
    let mut cells = cells.into_iter();
    let mut offset = 0;
    for (width, alignment) in widths.iter().zip(alignments) {
        row.line
            .spans
            .push(Span::styled("│ ", Style::new().fg(BORDER)));
        offset += "│ ".len();
        let cell = cells.next();
        offset +=
            append_cell(&mut row, cell, *width, *alignment, header, offset);
        row.line.spans.push(Span::raw(" "));
        offset += 1;
    }
    row.line
        .spans
        .push(Span::styled("│", Style::new().fg(BORDER)));
    row
}

fn append_cell(
    row: &mut Row,
    cell: Option<Cell>,
    width: usize,
    alignment: Alignment,
    header: bool,
    offset: usize,
) -> usize {
    let length = cell.as_ref().map_or(0, |cell| cell.text.width());
    let padding = width.saturating_sub(length);
    let left = match alignment {
        Alignment::Right => padding,
        Alignment::Center => padding / 2,
        _ => 0,
    };
    row.line.spans.push(Span::raw(" ".repeat(left)));
    let mut bytes = left;
    if let Some(cell) = cell {
        bytes += cell.text.len();
        for mut mapping in cell.mappings {
            mapping.display.start += offset + left;
            mapping.display.end += offset + left;
            row.mappings.push(mapping);
        }
        row.line
            .spans
            .extend(cell.spans.into_iter().map(|mut span| {
                if header {
                    span.style = span.style.add_modifier(Modifier::BOLD);
                    if span.style.fg == Some(TEXT) {
                        span.style = span.style.fg(AMBER);
                    }
                }
                span
            }));
    }
    row.line.spans.push(Span::raw(" ".repeat(padding - left)));
    bytes + padding - left
}
