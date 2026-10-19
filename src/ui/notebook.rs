//! Notebook panes paint only cached pages and prepared source context.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListState, Paragraph},
};

use crate::{
    app::{App, Focus, Notebook},
    content,
};

use super::{Areas, theme::*};

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, areas: &Areas) {
    let Some(notebook) = &app.notebook else {
        return;
    };
    if let Some(area) = areas.library {
        collections(frame, app, notebook, area);
    }
    if let Some(area) = areas.articles {
        notes(frame, app, notebook, area);
    }
    if let Some(area) = areas.reader {
        context(frame, app, notebook, area);
    }
    if let Some(area) = areas.annotations {
        backlinks(frame, app, notebook, area);
    }
}

fn collections(
    frame: &mut Frame<'_>,
    app: &App,
    notebook: &Notebook,
    area: Rect,
) {
    let block = panel(" Collections ".into(), app.focus == Focus::Library);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let items = ["All notes", "Pinned", "Open questions", "Follow-up actions"]
        .into_iter()
        .map(str::to_owned)
        .chain(notebook.collections.iter().map(|item| item.name.clone()))
        .map(|name| Line::raw(format!(" {name}")))
        .collect::<Vec<_>>();
    let mut state =
        ListState::default().with_selected(Some(notebook.collection));
    *state.offset_mut() = notebook.collection_start(inner.height);
    frame.render_stateful_widget(
        List::new(items)
            .style(Style::new().fg(TEXT))
            .highlight_style(Style::new().fg(AMBER).bg(RAISED))
            .highlight_symbol("▎"),
        inner,
        &mut state,
    );
}

fn notes(frame: &mut Frame<'_>, app: &App, notebook: &Notebook, area: Rect) {
    let title = format!(
        " Notes · {}{} ",
        notebook.page.total,
        if notebook.loading {
            " · updating…"
        } else {
            ""
        }
    );
    let block = panel(title, app.focus == Focus::Articles);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    if notebook.page.notes.is_empty() {
        empty_notes(frame, notebook.loading, inner);
        return;
    }
    let (first, capacity) = notebook.window(inner.height);
    for (row, note) in notebook
        .page
        .notes
        .iter()
        .enumerate()
        .skip(first)
        .take(capacity)
    {
        let rect = Rect::new(
            inner.x,
            inner.y + (row - first) as u16 * 5,
            inner.width,
            5.min(inner.height.saturating_sub((row - first) as u16 * 5)),
        );
        note_card(frame, notebook, row, note, rect);
    }
}

fn empty_notes(frame: &mut Frame<'_>, loading: bool, area: Rect) {
    let message = if loading {
        "\n  Searching notes…"
    } else {
        "\n  No matching notes.\n\n  / changes the query\n  Esc returns to reading"
    };
    frame.render_widget(
        Paragraph::new(message).style(Style::new().fg(MUTED)),
        area,
    );
}

fn note_card(
    frame: &mut Frame<'_>,
    notebook: &Notebook,
    index: usize,
    note: &crate::model::NoteSummary,
    area: Rect,
) {
    let selected = index == notebook.selected;
    let heading = note_heading(notebook, note, selected);
    let body = if note.body.trim().is_empty() {
        &note.quote
    } else {
        &note.body
    };
    let lines = vec![
        Line::styled(
            heading,
            Style::new().fg(AMBER).add_modifier(Modifier::BOLD),
        ),
        match_line(&note.preview, body),
        Line::styled(
            format!("  {}", content::label(&note.title, 120)),
            Style::new().fg(MUTED),
        ),
        Line::styled(
            format!(
                "  {}",
                note.tags
                    .iter()
                    .map(|tag| format!("#{tag}"))
                    .collect::<Vec<_>>()
                    .join(" ")
            ),
            Style::new().fg(BLUE),
        ),
    ];
    let style = Style::new().bg(if selected { RAISED } else { PANEL });
    frame.render_widget(Paragraph::new(lines).style(style), area);
}

fn note_heading(
    notebook: &Notebook,
    note: &crate::model::NoteSummary,
    selected: bool,
) -> String {
    let marker = if notebook.marked.contains(&note.id) {
        "✓"
    } else if selected {
        "▎"
    } else {
        " "
    };
    let state = if note.metadata.resolved {
        "Resolved"
    } else {
        "Open"
    };
    format!(
        "{marker} {}{} · {state} · #{}",
        if note.metadata.pinned { "★ " } else { "" },
        note.metadata.kind.name(),
        note.id
    )
}

fn context(frame: &mut Frame<'_>, app: &App, notebook: &Notebook, area: Rect) {
    let block = panel(" Context ".into(), app.focus == Focus::Reader);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(context) = notebook.current_context() else {
        let message = if notebook.context_loading {
            "\n  Loading source context…"
        } else {
            "\n  Select a note to see its source."
        };
        frame.render_widget(
            Paragraph::new(message).style(Style::new().fg(MUTED)),
            inner,
        );
        return;
    };
    let source = source_heading();
    let rows = context_headings(context)
        .into_iter()
        .chain(context.lines.iter().cloned())
        .chain(source)
        .chain(context.source_lines.iter().cloned())
        .skip(notebook.scroll)
        .take(inner.height as usize)
        .collect::<Vec<_>>();
    frame.render_widget(
        Paragraph::new(rows).style(Style::new().fg(TEXT)),
        inner,
    );
}

fn source_heading() -> [Line<'static>; 3] {
    [
        Line::raw(""),
        Line::styled("SOURCE CONTEXT", Style::new().fg(BLUE)),
        Line::raw(""),
    ]
}

pub(crate) fn context_scroll_limit(notebook: &Notebook, height: u16) -> usize {
    let Some(context) = notebook.current_context() else {
        return 0;
    };
    let rows = context_headings(context).len()
        + context.lines.len()
        + source_heading().len()
        + context.source_lines.len();
    rows.saturating_sub(usize::from(height.saturating_sub(2).max(1)))
}

fn context_headings(context: &crate::app::NoteContext) -> Vec<Line<'static>> {
    vec![
        Line::styled(
            content::label(&context.title, 160),
            Style::new().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Line::styled(
            format!(
                "Note #{} · {} · {}",
                context.note.comment.id,
                context.note.metadata.kind.name(),
                context.placement
            ),
            Style::new().fg(AMBER),
        ),
        Line::raw(""),
        Line::styled("YOUR NOTE", Style::new().fg(AMBER)),
    ]
}

fn backlinks(
    frame: &mut Frame<'_>,
    app: &App,
    notebook: &Notebook,
    area: Rect,
) {
    let block = panel(" Backlinks ".into(), app.focus == Focus::Annotations);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    let Some(context) = notebook.current_context() else {
        return;
    };
    let mut rows = vec![Line::styled(" Referenced by", Style::new().fg(AMBER))];
    rows.extend(
        context
            .backlinks
            .iter()
            .map(|note| reference_line(notebook, note)),
    );
    rows.push(Line::raw(""));
    rows.push(Line::styled(" References", Style::new().fg(AMBER)));
    rows.extend(
        context
            .references
            .iter()
            .map(|note| reference_line(notebook, note)),
    );
    let link_count = context.backlinks.len() + context.references.len();
    let row = notebook.link_selected
        + if notebook.link_selected < context.backlinks.len() {
            1
        } else {
            3
        };
    let selected = (link_count > 0).then_some(row);
    let mut state = ListState::default().with_selected(selected);
    frame.render_stateful_widget(
        List::new(rows)
            .style(Style::new().fg(TEXT))
            .highlight_style(Style::new().fg(AMBER).bg(RAISED)),
        inner,
        &mut state,
    );
}

fn match_line(preview: &str, fallback: &str) -> Line<'static> {
    if preview.is_empty() {
        return Line::styled(
            format!("  {}", content::label(fallback, 240)),
            Style::new().fg(TEXT),
        );
    }
    let mut spans = vec![Span::raw("  ")];
    for (index, segment) in preview.split('\u{1e}').enumerate() {
        if index == 0 {
            spans.push(Span::raw(content::clean(segment)));
        } else if let Some((matched, tail)) = segment.split_once('\u{1f}') {
            spans.push(Span::styled(
                content::clean(matched),
                Style::new().fg(AMBER).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(content::clean(tail)));
        }
    }
    Line::from(spans)
}

fn reference_line(
    notebook: &Notebook,
    note: &crate::model::NoteSummary,
) -> Line<'static> {
    let selected = notebook
        .current_context()
        .and_then(|context| {
            context
                .backlinks
                .iter()
                .chain(&context.references)
                .nth(notebook.link_selected)
        })
        .is_some_and(|selected| selected.id == note.id);
    Line::styled(
        format!(" #{} {}", note.id, content::label(&note.body, 100)),
        Style::new().fg(if selected { AMBER } else { TEXT }),
    )
}
