//! Visible note cards use cached source ordering, with no SQL or source parsing.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Paragraph},
};

use crate::{
    app::{AnnotationPosition, App, Focus},
    content,
};

use super::{hit::annotation_window, theme::*};

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, area: Rect) {
    let block = panel(
        format!(" Annotations · {} ", app.annotations.len()),
        app.focus == Focus::Annotations,
    );
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if app.annotations.is_empty() {
        frame.render_widget(
            Paragraph::new(concat!(
                "\n  Your reading, annotated.\n\n  v selects a passage\n",
                "  s saves a highlight\n  n adds a passage note\n\n",
                "  :note adds a general note"
            ))
            .wrap(ratatui::widgets::Wrap { trim: false })
            .style(Style::new().fg(MUTED)),
            inner,
        );
        return;
    }
    let (first, capacity) = annotation_window(app, inner.height);
    for (row, entry) in app
        .annotations
        .iter()
        .skip(first)
        .take(capacity)
        .enumerate()
    {
        let rect = Rect::new(
            inner.x,
            inner.y + row as u16 * 6,
            inner.width,
            6.min(inner.height.saturating_sub(row as u16 * 6)),
        );
        card(frame, app, entry, rect);
    }
}

fn card(
    frame: &mut Frame<'_>,
    app: &App,
    entry: &crate::app::AnnotationEntry,
    area: Rect,
) {
    let Some(note) = app.comments.get(entry.comment_index) else {
        return;
    };
    let selected = entry.comment_index == app.comment_selected;
    if selected {
        frame.render_widget(Block::new().style(Style::new().bg(RAISED)), area);
    }
    let position = match entry.position {
        AnnotationPosition::Located { percent, .. } => format!("{percent}%"),
        AnnotationPosition::OtherSource { .. } => "Other source".into(),
        AnnotationPosition::Unplaced => "Unplaced quote".into(),
        AnnotationPosition::General => "General note".into(),
    };
    let mut lines = vec![Line::styled(
        format!(
            "{} [{}] {} · {}",
            if selected { "▎" } else { " " },
            entry.comment_index + 1,
            position,
            note.source.as_str()
        ),
        Style::new().fg(AMBER).add_modifier(Modifier::BOLD),
    )];
    append_preview(&mut lines, &note.quote, area.width, BLUE);
    let body = if note.body.is_empty() {
        "Highlight"
    } else {
        &note.body
    };
    append_preview(&mut lines, body, area.width, TEXT);
    frame.render_widget(Paragraph::new(lines), area);
}

fn append_preview(
    lines: &mut Vec<Line<'static>>,
    text: &str,
    width: u16,
    color: ratatui::style::Color,
) {
    let safe = content::label(text, 500);
    for line in textwrap::wrap(&safe, width.saturating_sub(3).max(1) as usize)
        .iter()
        .take(2)
    {
        lines.push(Line::styled(format!("  {line}"), Style::new().fg(color)));
    }
}
