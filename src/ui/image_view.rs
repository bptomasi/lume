//! Native pixels stay outside scrolling panes and are painted after theming.

use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, Paragraph},
};

use crate::app::{ImageView, Mode};

use super::theme;
use crate::presentation::geometry::image_inspector_area;

const CONTROLS: &str = concat!(
    "+/- zoom · 0 fit · hjkl/arrows or wheel pan",
    " · o original · b blocks · Esc close"
);

pub(super) fn draw(frame: &mut Frame<'_>, view: &ImageView<'_>) {
    frame.render_widget(
        Block::new().style(Style::new().bg(theme::PANEL).fg(theme::TEXT)),
        frame.area(),
    );
    let Some(key) = view.image_inspector_key() else {
        return;
    };
    chrome(frame, view, &key);
    theme::apply(frame.buffer_mut(), view.theme);
    let column = image_inspector_area(frame.area());
    if column.is_empty() {
        return;
    }
    let Some(image) = view.images.thumbnail(&key) else {
        let message = view.images.placeholder(&key);
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
    if let Some(native) = &image.native {
        frame.render_widget(ratatui_image::Image::new(native), area);
    } else {
        super::images::paint_thumbnail(frame, area, 0, 0, &image.cells);
    }
}

fn title(view: &ImageView<'_>, key: &crate::media::ImageKey) -> Option<String> {
    let Mode::Image(image) = view.mode else {
        return None;
    };
    let dimensions =
        view.images
            .thumbnail(key)
            .map_or_else(String::new, |ready| {
                format!(
                    " · {}×{} source pixels",
                    ready.dimensions.0, ready.dimensions.1
                )
            });
    let protocol = key
        .detail
        .as_ref()
        .map_or("Blocks", |detail| detail.protocol.label());
    let kind = if image.formula.is_some() {
        "Formula"
    } else {
        "Image"
    };
    Some(format!(
        "{kind} · {}× · {protocol}{dimensions} · {}",
        image.zoom,
        crate::content::label(&image.title, 160),
    ))
}

fn chrome(
    frame: &mut Frame<'_>,
    view: &ImageView<'_>,
    key: &crate::media::ImageKey,
) {
    let Some(title) = title(view, key) else {
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
    let controls = if matches!(view.mode, Mode::Image(image) if image.formula.is_some())
    {
        "+/- zoom · 0 fit · hjkl/arrows or wheel pan · y copy LaTeX · Esc close"
    } else {
        CONTROLS
    };
    frame.render_widget(
        Paragraph::new(format!(
            "{controls}\nPixel backends preserve detail; blocks approximate the image."
        ))
        .style(Style::new().fg(theme::MUTED)),
        footer,
    );
}
