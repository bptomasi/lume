//! Visible selection/cursor overlays preserve cached syntax and saved marks.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::{
    app::{App, Focus, Mode},
    reader::{cursor, style_range},
};

use super::theme::{AMBER, BG, RAISED, TEXT};

pub(super) fn backgrounds(
    frame: &mut ratatui::Frame<'_>,
    app: &App,
    area: ratatui::layout::Rect,
) {
    for row in 0..usize::from(area.height) {
        if code_background(app, app.scroll + row) {
            frame.buffer_mut().set_style(
                ratatui::layout::Rect::new(
                    area.x,
                    area.y + row as u16,
                    area.width,
                    1,
                ),
                Style::new().bg(RAISED),
            );
        }
    }
}

pub(super) fn row(
    app: &App,
    index: usize,
    cached: &Line<'static>,
    selected: Option<&std::ops::Range<usize>>,
    width: usize,
) -> Line<'static> {
    if let Some(columns) =
        app.reader_columns.get(index).and_then(Option::as_ref)
    {
        return indexed_row(app, index, cached, selected, width, columns);
    }
    let mut line = cached.clone();
    paint_search(app, index, &mut line);
    let code = code_background(app, index);
    if code {
        line = line.style(Style::new().bg(RAISED));
    }
    if let Some(selected) = selected
        && let Some(range) = app.reader_ranges.get(index)
    {
        let start = range.start.max(selected.start);
        let end = range.end.min(selected.end);
        if start < end {
            style_range(
                &mut line,
                start - range.start..end - range.start,
                Style::new().bg(AMBER).fg(BG),
            );
        }
    }
    paint_cursor(app, index, &mut line);
    line
}

fn paint_search(app: &App, index: usize, line: &mut Line<'static>) {
    let Some(range) = app.reader_ranges.get(index) else {
        return;
    };
    let hits = &app.reader_tools.hits;
    let first = hits.partition_point(|hit| hit.end <= range.start);
    for hit in hits[first..].iter().take_while(|hit| hit.start < range.end) {
        let start = range.start.max(hit.start);
        let end = range.end.min(hit.end);
        style_range(
            line,
            start - range.start..end - range.start,
            Style::new().fg(AMBER).add_modifier(Modifier::UNDERLINED),
        );
    }
}

fn indexed_row(
    app: &App,
    row: usize,
    cached: &Line<'static>,
    selected: Option<&std::ops::Range<usize>>,
    width: usize,
    columns: &crate::reader::viewport::RowIndex,
) -> Line<'static> {
    let unwrapped = app.code_lines.get(row) == Some(&true);
    let code = code_background(app, row);
    let text = app.reader_lines.get(row).map_or("", String::as_str);
    let base = app.reader_ranges.get(row).map_or(0, |range| range.start);
    let cursor = (app.focus == Focus::Reader
        && matches!(app.mode, Mode::Normal)
        && row == app.reader_cursor)
        .then_some(base + app.reader_column);
    let mut line = columns.project(
        cached,
        text,
        if unwrapped {
            app.horizontal_for_row(row)
        } else {
            0
        },
        width,
        |bytes, style| {
            overlays(
                app,
                base + bytes.start..base + bytes.end,
                selected,
                cursor,
                style,
            )
        },
    );
    if code {
        line = line.style(Style::new().bg(RAISED));
    }
    if text.is_empty()
        && cursor.is_some()
        && (!unwrapped || app.horizontal_for_row(row) == 0)
    {
        line.spans.push(Span::styled(" ", cursor_style()));
    }
    line
}

fn overlays(
    app: &App,
    bytes: std::ops::Range<usize>,
    selected: Option<&std::ops::Range<usize>>,
    cursor: Option<usize>,
    mut style: Style,
) -> Style {
    let hits = &app.reader_tools.hits;
    let first = hits.partition_point(|hit| hit.end <= bytes.start);
    if hits.get(first).is_some_and(|hit| hit.start < bytes.end) {
        style = style.fg(AMBER).add_modifier(Modifier::UNDERLINED);
    }
    if selected
        .is_some_and(|range| range.start < bytes.end && range.end > bytes.start)
    {
        style = style.bg(AMBER).fg(BG);
    }
    if cursor.is_some_and(|cursor| bytes.contains(&cursor)) {
        style = style.patch(cursor_style());
    }
    style
}

pub(super) fn code_background(app: &App, row: usize) -> bool {
    let table = app.table_rows.partition_point(|range| range.end <= row);
    app.code_lines.get(row) == Some(&true)
        && !app
            .table_rows
            .get(table)
            .is_some_and(|range| range.contains(&row))
}

fn cursor_style() -> Style {
    Style::new()
        .bg(TEXT)
        .fg(BG)
        .add_modifier(Modifier::BOLD)
        .remove_modifier(Modifier::DIM | Modifier::REVERSED)
}

fn paint_cursor(app: &App, index: usize, line: &mut Line<'static>) {
    if app.focus == Focus::Reader
        && matches!(app.mode, Mode::Normal)
        && index == app.reader_cursor
    {
        // Paint the only reader cursor. A hardware block on this same cell
        // can invert it again, cancelling the intended contrast.
        let style = cursor_style();
        let text = app.reader_lines.get(index).map_or("", String::as_str);
        if text.is_empty() {
            line.spans.push(Span::styled(" ", style));
        } else {
            style_range(
                line,
                app.reader_column..cursor::next(text, app.reader_column),
                style,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use ratatui::{
        style::Color,
        text::{Line, Span},
    };

    #[test]
    fn clipping_limits_long_code_and_keeps_partial_wide_cells_aligned() {
        let line = Line::from(Span::styled(
            format!("\t界x{}", "a".repeat(100_000)),
            ratatui::style::Style::new().fg(Color::Blue),
        ));
        let text = line.to_string();
        let columns = crate::reader::viewport::RowIndex::new(&text, &line);
        let clipped = columns.project(&line, &text, 5, 4, |_, style| style);
        assert_eq!(clipped.to_string(), " xaa");
        assert_eq!(clipped.width(), 4);
        assert_eq!(clipped.spans[0].style.fg, Some(Color::Blue));
    }
}
