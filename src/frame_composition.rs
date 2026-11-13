//! Terminal frame sequencing around the application's bounded pane capture.
//!
//! Cached UI remains immutable. Capture must occur after themed panes/overlays
//! and before motion, hints on moving panes, and native image output. Image
//! inspection and linked readings retain their original early-return paths.

use crate::{
    app::{App, Mode},
    ui,
};
use ratatui::Frame;

/// Draw an interactive frame and capture bounded gesture pictures when needed.
pub fn draw_interactive(frame: &mut Frame<'_>, app: &mut App) {
    ui::draw_base(frame, &app.presentation());
    if matches!(app.mode(), Mode::Image(_)) {
        return;
    }
    if app.web_active() {
        ui::draw_native(frame, &app.foreground_presentation());
        return;
    }
    app.capture_pane_frame(frame.buffer_mut());
    ui::draw_foreground(frame, &app.foreground_presentation());
}
