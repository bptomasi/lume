//! Cached terminal rendering and bounded, optional pane-image capture.

mod annotations;
mod chrome;
mod command_line;
mod drag_picture;
mod hit;
mod image_view;
mod images;
pub(crate) use drag_picture::compact as compact_drag_picture;
pub use image_view::image_inspector_area;
mod key_guide;
mod layout;
mod navigation;
mod note_preview;
mod notebook;
pub(crate) use notebook::context_scroll_limit as notebook_scroll_limit;
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
pub use web::content as web_content;

pub(crate) use theme::image_background;
pub use welcome::draw as draw_welcome;

use ratatui::Frame;

use crate::app::App;

pub use hit::{annotation_at, article_at, navigation_at};
pub use layout::{
    Areas, SplitBoundary, areas, pane_buttons, pane_close_rect, pane_move_rect,
    picker_regions, reader_content, workspace_areas, workspace_reader_content,
};
pub(crate) use navigation::origin as navigation_origin;
pub use note_preview::note_preview_area;

/// Render cached rows; no SQL, HTTP, syntax parsing or Markdown layout occurs.
pub fn draw(frame: &mut Frame<'_>, app: &App) {
    draw_base(frame, app);
    pane_motion::draw(frame, app);
    images::draw_native(frame, app);
}

/// Capture bounded pane pictures before hints and terminal output.
pub fn draw_interactive(frame: &mut Frame<'_>, app: &mut App) {
    draw_base(frame, app);
    if matches!(app.mode, crate::app::Mode::Image(_)) || app.web_active() {
        return;
    }
    app.capture_pane_frame(frame.buffer_mut());
    pane_motion::draw(frame, app);
    images::draw_native(frame, app);
}

fn draw_base(frame: &mut Frame<'_>, app: &App) {
    if app.web_active() {
        web::draw(frame, app);
        if !matches!(app.mode, crate::app::Mode::Web) {
            overlays::draw(frame, app);
        }
        theme::apply(frame.buffer_mut(), &app.workspace.theme);
        return;
    }
    if matches!(app.mode, crate::app::Mode::Image(_)) {
        image_view::draw(frame, app);
        return;
    }
    draw_workspace(frame, app);
    theme::apply(frame.buffer_mut(), &app.workspace.theme);
}

fn draw_workspace(frame: &mut Frame<'_>, app: &App) {
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
    if !app.workspace.fullscreen {
        chrome::header(frame, app);
    }
    let areas = workspace_areas(frame.area(), app.focus, &app.workspace);
    if app.notebook.is_some() {
        notebook::draw(frame, app, &areas);
    } else {
        draw_reading_panes(frame, app, &areas);
    }
    chrome::pane_controls(frame, app, &areas);
    if !app.workspace.fullscreen {
        chrome::footer(frame, app);
    }
    if app.notebook.is_none() {
        note_preview::draw(frame, app);
    }
    overlays::draw(frame, app);
    key_guide::draw(frame, app);
}

fn draw_reading_panes(frame: &mut Frame<'_>, app: &App, areas: &Areas) {
    if let Some(area) = areas.library {
        panes::library(frame, app, area);
    }
    if let Some(area) = areas.articles {
        panes::articles(frame, app, area);
    }
    if let Some(area) = areas.reader {
        panes::reader(frame, app, area);
    }
    if let Some(area) = areas.annotations {
        annotations::draw(frame, app, area);
    }
}
