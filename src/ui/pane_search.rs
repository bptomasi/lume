//! Inline search fields reserve the same rows for drawing and pointer hits.

use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

use crate::{
    app::{Input, InputKind, Mode, SearchView},
    model::Pane,
};

use super::theme::{AMBER, MUTED};

fn editor<'a>(view: &SearchView<'a>, pane: Pane) -> Option<&'a Input> {
    let Mode::Input(input) = view.mode else {
        return None;
    };
    matches!(
        (pane, &input.kind),
        (Pane::Library, InputKind::LibrarySearch)
            | (Pane::Articles, InputKind::Search)
    )
    .then_some(input)
}

fn query<'a>(view: &SearchView<'a>, pane: Pane) -> &'a str {
    match pane {
        Pane::Library => view.navigation.library_query,
        Pane::Articles => view.navigation.search,
        _ => "",
    }
}

pub(super) fn content(view: &SearchView<'_>, pane: Pane, inner: Rect) -> Rect {
    crate::presentation::hit::search_content(
        inner,
        editor(view, pane).is_some() || !query(view, pane).is_empty(),
    )
}

pub(super) fn draw(
    frame: &mut Frame<'_>,
    view: &SearchView<'_>,
    pane: Pane,
    inner: Rect,
) -> Rect {
    let content = content(view, pane, inner);
    if content == inner {
        return content;
    }
    let field = Rect::new(inner.x, inner.y, inner.width, 1.min(inner.height));
    if let Some(input) = editor(view, pane) {
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
            Paragraph::new(format!("/ {}", query(view, pane)))
                .style(Style::new().fg(AMBER)),
            field,
        );
    }
    if inner.height > 1 {
        let hint = if editor(view, pane).is_some() {
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
