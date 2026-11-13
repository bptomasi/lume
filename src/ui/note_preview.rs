//! Bounded, nonmodal annotation previews contain only cached, safe note text.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Wrap},
};

use crate::app::{Mode, PreviewView, ReaderView};

use crate::presentation::geometry::note_preview_area;

use super::theme::{AMBER, BLUE, TEXT};

pub(super) fn area(view: &PreviewView<'_>, screen: Rect) -> Option<Rect> {
    let reading = view.reader;
    if !matches!(view.mode, Mode::Normal) || view.layout_prefix_pending {
        return None;
    }
    let (id, anchor) = reading.hovered_note()?;
    reading
        .comments
        .iter()
        .any(|note| note.id == id)
        .then(|| note_preview_area(screen, anchor))
}

pub(super) fn draw(frame: &mut Frame<'_>, view: &PreviewView<'_>) {
    let reading = view.reader;
    if !matches!(view.mode, Mode::Normal) || view.layout_prefix_pending {
        return;
    }
    let Some((id, anchor)) = reading.hovered_note() else {
        return;
    };
    let Some((index, note)) = reading
        .comments
        .iter()
        .enumerate()
        .find(|(_, note)| note.id == id)
    else {
        return;
    };
    let area = note_preview_area(frame.area(), anchor);
    let inner = super::popup::draw_frame(
        frame,
        &format!("Note {} · {}", index + 1, note.source.as_str()),
        area,
    );
    let height = inner.height.saturating_sub(1);
    let text = preview_lines(note, inner.width, height);
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
    height: u16,
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
    let end = body.floor_char_boundary(body.len().min(4096));
    lines.extend(
        crate::content::clean(&body[..end])
            .lines()
            .take(usize::from(height).saturating_sub(lines.len()))
            .map(|line| Line::raw(line.to_owned())),
    );
    lines
}

pub(super) fn markers(
    frame: &mut Frame<'_>,
    reading: ReaderView<'_>,
    column: Rect,
) {
    for index in reading.scroll..reading.scroll + usize::from(column.height) {
        let ids = reading.note_markers(index);
        if ids.is_empty() {
            continue;
        }
        let glyph = if ids.len() > 1 { "◉" } else { "●" };
        frame.render_widget(
            Paragraph::new(glyph)
                .style(Style::new().fg(AMBER).add_modifier(Modifier::BOLD)),
            Rect::new(
                column.x.saturating_sub(1),
                column.y + (index - reading.scroll) as u16,
                1,
                1,
            ),
        );
    }
}
