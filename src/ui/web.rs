//! Linked readings paint prepared rows without borrowing article mutation state.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::app::{App, WebPage};

use super::theme::{BG, MUTED, PANEL, RAISED, TEXT};

/// Centered linked-reading column shared by rendering and pointer input.
pub fn content(screen: Rect, width: usize) -> Rect {
    let width = (width as u16).min(screen.width.saturating_sub(4));
    Rect::new(
        screen.x + screen.width.saturating_sub(width) / 2,
        screen.y + screen.height.min(3),
        width,
        screen.height.saturating_sub(6),
    )
}

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    let screen = frame.area();
    let title = app
        .web_page()
        .map_or("Linked page", |page| &page.document.title);
    super::popup::draw_frame(frame, title, screen);
    let Some(page) = app.web_page() else {
        frame.render_widget(
            Paragraph::new(app.status.as_str()).style(Style::new().fg(TEXT)),
            content(screen, app.workspace.reading_width.into()),
        );
        return;
    };
    let area = content(screen, page.width);
    frame.render_widget(
        Paragraph::new(page.document.url.as_str())
            .style(Style::new().fg(MUTED)),
        Rect::new(area.x, screen.y + 1, area.width, 1),
    );
    let lines = page
        .rendered
        .lines
        .iter()
        .enumerate()
        .skip(page.scroll)
        .take(usize::from(area.height))
        .map(|(index, line)| row(page, index, line, area.width.into()))
        .collect::<Vec<_>>();
    draw_code_backgrounds(frame, page, area);
    frame.render_widget(Paragraph::new(lines), area);
    draw_controls(frame, screen, &app.status);
}

fn draw_code_backgrounds(frame: &mut Frame<'_>, page: &WebPage, area: Rect) {
    for (row, code) in page
        .rendered
        .code
        .iter()
        .skip(page.scroll)
        .take(usize::from(area.height))
        .enumerate()
    {
        if *code {
            frame.render_widget(
                ratatui::widgets::Block::new().style(Style::new().bg(RAISED)),
                Rect::new(area.x, area.y + row as u16, area.width, 1),
            );
        }
    }
}

fn draw_controls(frame: &mut Frame<'_>, screen: Rect, status: &str) {
    frame.render_widget(
        Paragraph::new(status).style(Style::new().fg(MUTED)),
        Rect::new(
            screen.x + 2,
            screen.bottom().saturating_sub(3),
            screen.width.saturating_sub(4),
            1,
        ),
    );
    frame.render_widget(
        Paragraph::new(concat!(
            "hjkl · w/b/e · v/y · Enter link · Ctrl-o back · ",
            ":save-page · o browser"
        ))
        .style(Style::new().fg(MUTED)),
        Rect::new(
            screen.x + 2,
            screen.bottom().saturating_sub(2),
            screen.width.saturating_sub(4),
            1,
        ),
    );
}

fn row(
    page: &WebPage,
    index: usize,
    line: &Line<'static>,
    width: usize,
) -> Line<'static> {
    let range = &page.rendered.text_ranges[index];
    let text = &page.rows[index];
    let cursor_row = page.cursor_row() == index;
    let selected = page.selected_range();
    let mut result = page.columns[index].project(
        line,
        text,
        page.horizontal(index),
        width,
        |bytes, mut style| {
            let source = range.start + bytes.start..range.start + bytes.end;
            if selected.as_ref().is_some_and(|selection| {
                source.start < selection.end && source.end > selection.start
            }) {
                style = style.bg(super::theme::AMBER).fg(BG);
            }
            if cursor_row && source.contains(&page.cursor) {
                style = style.bg(TEXT).fg(PANEL);
            }
            style
        },
    );
    if page.rendered.code.get(index) == Some(&true) {
        result = result.style(Style::new().bg(RAISED));
    }
    if cursor_row && text.is_empty() {
        result
            .spans
            .push(Span::styled(" ", Style::new().bg(TEXT).fg(PANEL)));
    }
    result
}
