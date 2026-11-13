//! Paint only intersecting cached image cells; never resize or decode in draw.

use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    widgets::Paragraph,
};

use crate::app::{ImageView, NativeImagesView};

pub(super) fn draw(frame: &mut Frame<'_>, view: &ImageView<'_>, column: Rect) {
    let viewport = view.viewport;
    for slot in viewport.visible() {
        let Some(key) = view.image_key(slot) else {
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
        if let Some(image) = view.images.thumbnail(&key) {
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
            let status = view.images.placeholder(&key);
            message(frame, column, viewport.scroll, slot.rows.start, status);
        }
    }
    paint_cursor(frame, view, column);
}

pub(super) fn draw_native(frame: &mut Frame<'_>, view: &NativeImagesView<'_>) {
    if !view.images.uses_sixel_images()
        || matches!(view.images.mode, crate::app::Mode::Image(_))
        || view.motion_active
        || (!view.images.web_active && view.notebook)
        || (!view.images.web_active && view.preparing && !view.stale)
        || frame.area().width < 24
        || frame.area().height < 10
    {
        return;
    }
    let Some(column) = native_column(view, frame.area()) else {
        return;
    };
    let occlusions = [
        super::popup::Popup::from_mode(view.images.mode)
            .map(|popup| popup.area(frame.area())),
        super::key_guide::area(&view.guide, frame.area()),
        super::note_preview::area(&view.preview, frame.area()),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let viewport = view.images.viewport;
    for slot in viewport.visible() {
        let Some(key) = view.images.image_key(slot) else {
            continue;
        };
        if let Some(image) = view.images.images.thumbnail(&key)
            && let Some(rows) = &image.rows
            && let Some((area, first)) =
                native_area(viewport.scroll, slot, column, image)
        {
            rows.render_uncovered(area, first, frame.buffer_mut(), &occlusions);
        }
    }
}

fn native_column(view: &NativeImagesView<'_>, screen: Rect) -> Option<Rect> {
    if view.images.web_active {
        return matches!(view.images.mode, crate::app::Mode::Web)
            .then(|| super::web_content(screen, view.images.viewport.width));
    }
    let reader =
        super::workspace_areas(screen, view.focus, view.workspace).reader?;
    Some(super::workspace_reader_content(reader, view.workspace))
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

fn paint_cursor(frame: &mut Frame<'_>, view: &ImageView<'_>, column: Rect) {
    let viewport = view.viewport;
    let row = if view.web_active {
        let Some(page) = view.web_page else {
            return;
        };
        if !matches!(view.mode, crate::app::Mode::Web) {
            return;
        }
        page.cursor_row()
    } else {
        if view.focus != crate::app::Focus::Reader
            || !matches!(view.mode, crate::app::Mode::Normal)
        {
            return;
        }
        view.cursor
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
