//! Bounded, nonmodal annotation previews contain only cached, safe note text.

use ratatui::{
    Frame,
    layout::{Position, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Clear, Paragraph, Wrap},
};

use crate::app::{App, Mode};

use super::theme::{AMBER, BLUE, PANEL, TEXT, panel};

/// Popup rectangle shared by drawing and hit testing, clamped to the terminal.
pub fn note_preview_area(screen: Rect, anchor: Position) -> Rect {
    let width = screen.width.saturating_sub(2).min(56);
    let height = screen.height.saturating_sub(2).min(9);
    let x = anchor
        .x
        .saturating_add(2)
        .min(screen.right().saturating_sub(width + 1))
        .max(screen.x);
    let y = if anchor.y.saturating_add(height + 1) < screen.bottom() {
        anchor.y + 1
    } else {
        anchor.y.saturating_sub(height)
    };
    Rect::new(
        x,
        y.clamp(screen.y, screen.bottom().saturating_sub(height)),
        width,
        height,
    )
}

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    if !matches!(app.mode, Mode::Normal) || app.layout_prefix_pending() {
        return;
    }
    let Some((id, anchor)) = app.hovered_note() else {
        return;
    };
    let Some((index, note)) = app
        .comments
        .iter()
        .enumerate()
        .find(|(_, note)| note.id == id)
    else {
        return;
    };
    let area = note_preview_area(frame.area(), anchor);
    let block = panel(
        format!(" Note {} · {} ", index + 1, note.source.as_str()),
        true,
    );
    let inner = block.inner(area);
    frame.render_widget(Clear, area);
    frame.render_widget(block.style(Style::new().bg(PANEL)), area);
    let text = preview_lines(note, inner.width);
    let height = inner.height.saturating_sub(1);
    frame.render_widget(
        Paragraph::new(text)
            .style(Style::new().fg(TEXT))
            .wrap(Wrap { trim: false }),
        Rect::new(inner.x, inner.y, inner.width, height),
    );
    frame.render_widget(
        Paragraph::new("Click to edit · :annotations shows all")
            .style(Style::new().fg(AMBER)),
        Rect::new(inner.x, inner.y + height, inner.width, 1),
    );
}

fn preview_lines(
    note: &crate::model::Comment,
    width: u16,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if !note.quote.is_empty() {
        for line in textwrap::wrap(
            &crate::content::label(&note.quote, 400),
            usize::from(width.max(1)),
        )
        .into_iter()
        .take(2)
        {
            lines.push(Line::styled(line.into_owned(), Style::new().fg(BLUE)));
        }
        lines.push(Line::raw(""));
    }
    let body = if note.body.is_empty() {
        "Highlight"
    } else {
        &note.body
    };
    lines.extend(
        crate::content::clean(body)
            .lines()
            .map(|line| Line::raw(line.to_owned())),
    );
    lines
}

pub(super) fn markers(frame: &mut Frame<'_>, app: &App, column: Rect) {
    for index in app.scroll..app.scroll + usize::from(column.height) {
        let ids = app.reader_note_markers(index);
        if ids.is_empty() {
            continue;
        }
        let glyph = if ids.len() > 1 { "◉" } else { "●" };
        frame.render_widget(
            Paragraph::new(glyph)
                .style(Style::new().fg(AMBER).add_modifier(Modifier::BOLD)),
            Rect::new(
                column.x.saturating_sub(1),
                column.y + (index - app.scroll) as u16,
                1,
                1,
            ),
        );
    }
}
