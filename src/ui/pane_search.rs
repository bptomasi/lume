//! Inline search fields reserve the same rows for drawing and pointer hits.

use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

use crate::{
    app::{App, Input, InputKind, Mode},
    model::Pane,
};

use super::theme::{AMBER, MUTED};

fn editor(app: &App, pane: Pane) -> Option<&Input> {
    let Mode::Input(input) = &app.mode else {
        return None;
    };
    matches!(
        (pane, &input.kind),
        (Pane::Library, InputKind::LibrarySearch)
            | (Pane::Articles, InputKind::Search)
    )
    .then_some(input)
}

fn query(app: &App, pane: Pane) -> &str {
    match pane {
        Pane::Library => app.library_query(),
        Pane::Articles => &app.search,
        _ => "",
    }
}

pub(super) fn content(app: &App, pane: Pane, inner: Rect) -> Rect {
    let rows = if editor(app, pane).is_some() || !query(app, pane).is_empty() {
        inner.height.min(2)
    } else {
        0
    };
    Rect::new(inner.x, inner.y + rows, inner.width, inner.height - rows)
}

pub(super) fn draw(
    frame: &mut Frame<'_>,
    app: &App,
    pane: Pane,
    inner: Rect,
) -> Rect {
    let content = content(app, pane, inner);
    if content == inner {
        return content;
    }
    let field = Rect::new(inner.x, inner.y, inner.width, 1.min(inner.height));
    if let Some(input) = editor(app, pane) {
        frame.render_widget(
            Paragraph::new("/ ").style(Style::new().fg(AMBER)),
            field,
        );
        frame.render_widget(
            &input.editor,
            Rect::new(
                field.x + field.width.min(2),
                field.y,
                field.width.saturating_sub(2),
                field.height,
            ),
        );
    } else {
        frame.render_widget(
            Paragraph::new(format!("/ {}", query(app, pane)))
                .style(Style::new().fg(AMBER)),
            field,
        );
    }
    if inner.height > 1 {
        let hint = if editor(app, pane).is_some() {
            "↑↓ matches · Enter apply · Esc cancel"
        } else {
            "/ edit · Esc clear"
        };
        frame.render_widget(
            Paragraph::new(hint).style(Style::new().fg(MUTED)),
            Rect::new(inner.x, inner.y + 1, inner.width, 1),
        );
    }
    content
}
