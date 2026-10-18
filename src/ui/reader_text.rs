//! Visible selection/cursor overlays preserve cached syntax and saved marks.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    app::{App, Focus, Mode},
    reader::{cursor, style_range},
};

use super::theme::{AMBER, BG, RAISED, TEXT};

pub(super) fn row(
    app: &App,
    index: usize,
    cached: &Line<'static>,
    selected: Option<&std::ops::Range<usize>>,
    width: usize,
) -> Line<'static> {
    let mut line = cached.clone();
    paint_search(app, index, &mut line);
    let code = app.code_lines.get(index) == Some(&true);
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
    if !code && !line.spans.iter().any(|span| span.content.contains('\t')) {
        return line;
    }
    scroll(line, if code { app.horizontal } else { 0 }, width)
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

fn scroll(line: Line<'static>, skip: usize, width: usize) -> Line<'static> {
    let mut column = 0;
    let end = skip.saturating_add(width);
    let mut spans = Vec::new();
    for span in line.spans {
        if column >= end {
            break;
        }
        let mut text = String::new();
        for grapheme in span.content.graphemes(true) {
            if column >= end {
                break;
            }
            let next = column + cursor::width(grapheme, column);
            append_visible(&mut text, grapheme, column..next, skip..end);
            column = next;
        }
        if !text.is_empty() {
            spans.push(Span::styled(text, span.style));
        }
    }
    Line::from(spans).style(line.style)
}

fn append_visible(
    text: &mut String,
    grapheme: &str,
    cells: std::ops::Range<usize>,
    viewport: std::ops::Range<usize>,
) {
    if cells.end <= viewport.start {
        return;
    }
    if grapheme == "\t"
        || cells.start < viewport.start
        || cells.end > viewport.end
    {
        let visible =
            cells.end.min(viewport.end) - cells.start.max(viewport.start);
        text.push_str(&" ".repeat(visible));
    } else {
        text.push_str(grapheme);
    }
}

fn paint_cursor(app: &App, index: usize, line: &mut Line<'static>) {
    if app.focus == Focus::Reader
        && matches!(app.mode, Mode::Normal)
        && index == app.reader_cursor
    {
        // Paint the only reader cursor. A hardware block on this same cell
        // can invert it again, cancelling contrast in terminals like Alacritty.
        let style = Style::new()
            .bg(TEXT)
            .fg(BG)
            .add_modifier(Modifier::BOLD)
            .remove_modifier(Modifier::DIM | Modifier::REVERSED);
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
        let clipped = super::scroll(line, 5, 4);
        assert_eq!(clipped.to_string(), " xaa");
        assert_eq!(clipped.width(), 4);
        assert_eq!(clipped.spans[0].style.fg, Some(Color::Blue));
    }
}
