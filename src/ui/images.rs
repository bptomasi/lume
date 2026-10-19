//! Paint only intersecting cached image cells; never resize or decode in draw.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::app::App;

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, column: Rect) {
    let bottom = app.scroll + usize::from(column.height);
    for slot in app
        .images
        .slots
        .iter()
        .filter(|slot| slot.rows.end > app.scroll && slot.caption < bottom)
    {
        let Some(key) = app.image_key(slot) else {
            message(
                frame,
                column,
                app.scroll,
                slot.rows.start,
                "Image URL unavailable",
            );
            continue;
        };
        if let Some(image) = app.images.thumbnail(&key) {
            paint_thumbnail(
                frame,
                column,
                app.scroll,
                slot.rows.start,
                &image.cells,
            );
        } else {
            let status = app.images.error(&key).unwrap_or({
                if app.images.enabled {
                    "Loading image…"
                } else {
                    "Image loading paused"
                }
            });
            message(frame, column, app.scroll, slot.rows.start, status);
        }
    }
    paint_cursor(frame, app, column);
}

fn paint_cursor(frame: &mut Frame<'_>, app: &App, column: Rect) {
    if app.focus != crate::app::Focus::Reader
        || !matches!(app.mode, crate::app::Mode::Normal)
        || app.reader_cursor < app.scroll
        || app.reader_cursor >= app.scroll + usize::from(column.height)
        || !app
            .images
            .slots
            .iter()
            .any(|slot| slot.rows.contains(&app.reader_cursor))
    {
        return;
    }
    let y = column.y + (app.reader_cursor - app.scroll) as u16;
    if let Some(cell) = frame.buffer_mut().cell_mut((column.x, y)) {
        cell.set_symbol(" ").set_style(
            Style::new()
                .bg(super::theme::TEXT)
                .fg(super::theme::BG)
                .add_modifier(Modifier::BOLD),
        );
    }
}

fn paint_thumbnail(
    frame: &mut Frame<'_>,
    column: Rect,
    scroll: usize,
    start: usize,
    cells: &ratatui::buffer::Buffer,
) {
    let bottom = scroll + usize::from(column.height);
    let left = column.x + column.width.saturating_sub(cells.area.width) / 2;
    for row in 0..usize::from(cells.area.height) {
        let display = start + row;
        if display < scroll || display >= bottom {
            continue;
        }
        for x in 0..cells.area.width.min(column.width) {
            let target = (left + x, column.y + (display - scroll) as u16);
            frame.buffer_mut()[target] = cells[(x, row as u16)].clone();
        }
    }
}

fn message(
    frame: &mut Frame<'_>,
    column: Rect,
    scroll: usize,
    row: usize,
    text: &str,
) {
    let visible = row.max(scroll);
    if visible >= scroll + usize::from(column.height) {
        return;
    }
    let area = Rect::new(
        column.x,
        column.y + (visible - scroll) as u16,
        column.width,
        1,
    );
    frame.render_widget(
        Paragraph::new(crate::content::label(text, 300))
            .style(Style::new().fg(super::theme::MUTED)),
        area,
    );
}
