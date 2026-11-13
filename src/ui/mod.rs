//! Render borrowed cached presentation; terminal composition owns capture.

mod annotations;
mod chrome;
mod command_line;
mod hit;
mod image_view;
mod images;
mod key_guide;
mod navigation;
mod note_preview;
mod notebook;
mod overlays;
mod pane_motion;
mod pane_search;
mod panes;
mod popup;
mod reader_text;
mod selectors;
mod theme;
mod web;
mod welcome;

pub use welcome::draw as draw_welcome;

use ratatui::Frame;

use crate::app::{ForegroundView, PresentationView};

pub use crate::presentation::geometry::{
    Areas, SplitBoundary, areas, image_inspector_area, note_preview_area,
    pane_buttons, pane_close_rect, pane_move_rect, picker_regions,
    reader_content, web_content, workspace_areas, workspace_reader_content,
};
pub use crate::presentation::hit::{
    article_hit as article_at, navigation_hit as navigation_at,
};

/// Render cached rows without SQL, HTTP, parsing or mutable application access.
pub fn draw(frame: &mut Frame<'_>, view: &PresentationView<'_>) {
    draw_base(frame, view);
    draw_foreground(frame, &view.foreground);
}

/// Paint cached panes and overlays before the terminal captures pane pictures.
pub fn draw_base(frame: &mut Frame<'_>, view: &PresentationView<'_>) {
    if matches!(view.overlays.mode, crate::app::Mode::Image(_)) {
        image_view::draw(frame, &view.foreground.native.images);
        return;
    }
    if view.web.images.web_active {
        web::draw(frame, &view.web);
        if !matches!(view.overlays.mode, crate::app::Mode::Web) {
            overlays::draw(frame, &view.overlays);
        }
        theme::apply(frame.buffer_mut(), &view.layout.workspace.theme);
        return;
    }
    draw_workspace(frame, view);
    theme::apply(frame.buffer_mut(), &view.layout.workspace.theme);
}

fn draw_workspace(frame: &mut Frame<'_>, view: &PresentationView<'_>) {
    use ratatui::{style::Style, widgets::Block};
    frame.render_widget(
        Block::new().style(Style::new().bg(theme::PANEL)),
        frame.area(),
    );
    if frame.area().width < 24 || frame.area().height < 10 {
        frame.render_widget(
            ratatui::widgets::Paragraph::new("Lume · resize terminal\nq exits"),
            frame.area(),
        );
        return;
    }
    if !view.layout.workspace.fullscreen {
        chrome::header(frame, &view.chrome);
    }
    let areas =
        workspace_areas(frame.area(), view.layout.focus, view.layout.workspace);
    if let Some(notebook) = &view.notebook {
        notebook::draw(frame, view.layout.focus, notebook, &areas);
    } else {
        draw_reading_panes(frame, view, &areas);
    }
    chrome::pane_controls(frame, &view.controls, &areas);
    if !view.layout.workspace.fullscreen {
        chrome::footer(frame, &view.chrome);
    }
    if view.notebook.is_none() {
        note_preview::draw(frame, &view.preview);
    }
    overlays::draw(frame, &view.overlays);
    key_guide::draw(frame, &view.guide);
}

fn draw_reading_panes(
    frame: &mut Frame<'_>,
    view: &PresentationView<'_>,
    areas: &Areas,
) {
    if let Some(area) = areas.library {
        panes::library(frame, &view.navigation, area);
    }
    if let Some(area) = areas.articles {
        panes::articles(frame, &view.navigation, area);
    }
    if let Some(area) = areas.reader {
        panes::reader(frame, &view.reader, area);
    }
    if let Some(area) = areas.annotations {
        annotations::draw(frame, view.reader.reader, view.layout.focus, area);
    }
}

/// Paint captured motion and prepared native images after terminal capture.
pub fn draw_foreground(frame: &mut Frame<'_>, view: &ForegroundView<'_>) {
    pane_motion::draw(frame, &view.motion);
    images::draw_native(frame, &view.native);
}

/// Paint prepared native images for a linked-reading frame without capture.
pub fn draw_native(frame: &mut Frame<'_>, view: &ForegroundView<'_>) {
    images::draw_native(frame, &view.native);
}
