//! Paint only intersecting cached image cells; never resize or decode in draw.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::app::App;

pub(super) fn draw(frame: &mut Frame<'_>, app: &App, column: Rect) {
    let viewport = app.image_viewport();
    for slot in viewport.visible() {
        let Some(key) = app.image_key(slot) else {
            if slot.formula.is_some() {
                continue;
            }
            message(
                frame,
                column,
                viewport.scroll,
                slot.rows.start,
                "Image URL unavailable",
            );
            continue;
        };
        if let Some(image) = app.images.thumbnail(&key) {
            if slot.formula.is_some() {
                clear_formula_caption(
                    frame,
                    column,
                    viewport.scroll,
                    slot.caption,
                );
            }
            if image.rows.is_some() {
                continue;
            }
            paint_thumbnail(
                frame,
                column,
                viewport.scroll,
                slot.rows.start,
                &image.cells,
            );
        } else if slot.formula.is_none() {
            let status = app.images.placeholder(&key);
            message(frame, column, viewport.scroll, slot.rows.start, status);
        }
    }
    paint_cursor(frame, app, column);
}

pub(super) fn draw_native(frame: &mut Frame<'_>, app: &App) {
    if matches!(app.mode, crate::app::Mode::Image(_))
        || app.pane_motion_active()
        || (!app.web_active() && app.notebook.is_some())
        || (!app.web_active() && app.article_preparing() && !app.reader_stale())
        || frame.area().width < 24
        || frame.area().height < 10
    {
        return;
    }
    let Some(column) = native_column(app, frame.area()) else {
        return;
    };
    let occlusions = [
        super::popup::Popup::from_mode(&app.mode)
            .map(|popup| popup.area(frame.area())),
        super::key_guide::area(app, frame.area()),
        super::note_preview::area(app, frame.area()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let viewport = app.image_viewport();
    for slot in viewport.visible() {
        let Some(key) = app.image_key(slot) else {
            continue;
        };
        if let Some(image) = app.images.thumbnail(&key)
            && let Some(rows) = &image.rows
            && let Some((area, first)) =
                native_area(viewport.scroll, slot, column, image)
        {
            rows.render_uncovered(area, first, frame.buffer_mut(), &occlusions);
        }
    }
}

fn native_column(app: &App, screen: Rect) -> Option<Rect> {
    if app.web_active() {
        return matches!(app.mode, crate::app::Mode::Web)
            .then(|| super::web_content(screen, app.image_viewport().width));
    }
    let reader =
        super::workspace_areas(screen, app.focus, &app.workspace).reader?;
    Some(super::workspace_reader_content(reader, &app.workspace))
}

fn clear_formula_caption(
    frame: &mut Frame<'_>,
    column: Rect,
    scroll: usize,
    caption: usize,
) {
    if caption < scroll || caption >= scroll + usize::from(column.height) {
        return;
    }
    let y = column.y + (caption - scroll) as u16;
    for x in column.x..column.right() {
        if let Some(cell) = frame.buffer_mut().cell_mut((x, y)) {
            cell.set_symbol(" ");
        }
    }
}

fn native_area(
    scroll: usize,
    slot: &crate::reader::images::ImageSlot,
    column: Rect,
    image: &crate::media::Thumbnail,
) -> Option<(Rect, u16)> {
    let first = scroll.saturating_sub(slot.rows.start);
    let last = (scroll + usize::from(column.height))
        .saturating_sub(slot.rows.start)
        .min(usize::from(image.cells.area.height));
    if first >= last
        || column.is_empty()
        || image.cells.area.width > column.width
    {
        return None;
    }
    let area = Rect::new(
        column.x + column.width.saturating_sub(image.cells.area.width) / 2,
        column.y + slot.rows.start.saturating_sub(scroll) as u16,
        image.cells.area.width,
        (last - first) as u16,
    );
    Some((area, first as u16))
}

fn paint_cursor(frame: &mut Frame<'_>, app: &App, column: Rect) {
    let viewport = app.image_viewport();
    let row = if app.web_active() {
        let Some(page) = app.web_page() else {
            return;
        };
        if !matches!(app.mode, crate::app::Mode::Web) {
            return;
        }
        page.cursor_row()
    } else {
        if app.focus != crate::app::Focus::Reader
            || !matches!(app.mode, crate::app::Mode::Normal)
        {
            return;
        }
        app.reader_cursor
    };
    if row < viewport.scroll
        || row >= viewport.scroll + usize::from(column.height)
        || !viewport
            .visible()
            .iter()
            .any(|slot| slot.rows.contains(&row))
    {
        return;
    }
    let y = column.y + (row - viewport.scroll) as u16;
    if let Some(cell) = frame.buffer_mut().cell_mut((column.x, y)) {
        cell.set_symbol(" ").set_style(
            Style::new()
                .bg(super::theme::TEXT)
                .fg(super::theme::BG)
                .add_modifier(Modifier::BOLD),
        );
    }
}

pub(super) fn paint_thumbnail(
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
