//! Native pixels stay outside scrolling panes and are painted after theming.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph},
};

use crate::app::{App, Mode};

use super::theme;

const CONTROLS: &str = concat!(
    "+/- zoom · 0 fit · hjkl/arrows or wheel pan",
    " · o original · b blocks · Esc close"
);

/// Bounded inspector viewport; the surrounding rows own title and controls.
pub fn image_inspector_area(screen: Rect) -> Rect {
    let width = screen.width.saturating_sub(4).min(160);
    let height = screen.height.saturating_sub(6).min(48);
    Rect::new(
        screen.x + screen.width.saturating_sub(width) / 2,
        screen.y + screen.height.saturating_sub(height) / 2,
        width,
        height,
    )
}

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    frame.render_widget(
        Block::new().style(Style::new().bg(theme::PANEL).fg(theme::TEXT)),
        frame.area(),
    );
    let Some(key) = app.image_inspector_key() else {
        return;
    };
    chrome(frame, app, &key);
    theme::apply(frame.buffer_mut(), &app.workspace.theme);
    let column = image_inspector_area(frame.area());
    if column.is_empty() {
        return;
    }
    let Some(image) = app.images.thumbnail(&key) else {
        let message = app.images.placeholder(&key);
        frame.render_widget(
            Paragraph::new(crate::content::label(message, 300)),
            column,
        );
        return;
    };
    let area = Rect::new(
        column.x + column.width.saturating_sub(image.cells.area.width) / 2,
        column.y + column.height.saturating_sub(image.cells.area.height) / 2,
        image.cells.area.width.min(column.width),
        image.cells.area.height.min(column.height),
    );
    if image.overlay.is_some() {
        return;
    }
    if let Some(native) = &image.native {
        frame.render_widget(ratatui_image::Image::new(native), area);
    } else {
        super::images::paint_thumbnail(frame, area, 0, 0, &image.cells);
    }
}

fn title(app: &App, key: &crate::media::ImageKey) -> Option<String> {
    let Mode::Image(image) = &app.mode else {
        return None;
    };
    let dimensions =
        app.images.thumbnail(key).map_or_else(String::new, |ready| {
            format!(
                " · {}×{} source pixels",
                ready.dimensions.0, ready.dimensions.1
            )
        });
    let protocol = key
        .detail
        .as_ref()
        .map_or("Blocks", |detail| detail.protocol.label());
    Some(format!(
        "Image · {}× · {protocol}{dimensions} · {}",
        image.zoom,
        crate::content::label(&image.title, 160),
    ))
}

fn chrome(frame: &mut Frame<'_>, app: &App, key: &crate::media::ImageKey) {
    let Some(title) = title(app, key) else {
        return;
    };
    let screen = frame.area();
    let heading =
        Rect::new(screen.x, screen.y, screen.width, 2.min(screen.height));
    frame.render_widget(
        Paragraph::new(title).style(Style::new().fg(theme::AMBER)),
        heading,
    );
    let footer = Rect::new(
        screen.x,
        screen.bottom().saturating_sub(2),
        screen.width,
        2.min(screen.height),
    );
    frame.render_widget(
        Paragraph::new(format!(
            "{CONTROLS}\n{}",
            app.images.notice().unwrap_or(
                "Pixel backends preserve detail; blocks approximate the image."
            )
        ))
        .style(Style::new().fg(theme::MUTED)),
        footer,
    );
}
