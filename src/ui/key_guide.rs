//! Prefix help is a transient overlay, independent of persistent pane geometry.

use ratatui::{Frame, layout::Rect, style::Style, widgets::Paragraph};

use crate::app::{App, Mode};

use super::theme::TEXT;

pub(super) fn draw(frame: &mut Frame<'_>, app: &App) {
    let Some(area) = area(app, frame.area()) else {
        return;
    };
    let width = area.width;
    let title = format!("Layout · Ctrl-w · {}", app.focus.title());
    let inner = super::popup::draw_frame(frame, &title, area);
    let help = if width < 62 {
        concat!(
            " h/j/k/l Focus · w Cycle\n",
            " H/J/K/L Move pane\n",
            " s Split below · v Split right\n",
            " +/- Resize · c Collapse\n",
            " a Open annotations\n",
            " u/r Undo / redo layout\n",
            " Esc Cancel"
        )
    } else {
        concat!(
            " h j k l   Focus a pane       H J K L   Move a pane\n",
            " s         Split below        v         Split right\n",
            " + / -     Resize split       c         Collapse\n",
            " a         Open annotations   w         Cycle focus\n",
            " u / r     Undo / redo layout\n\n",
            " :workspace load Study        Esc       Cancel"
        )
    };
    frame.render_widget(
        Paragraph::new(help).style(Style::new().fg(TEXT)),
        inner,
    );
}

pub(super) fn area(app: &App, screen: Rect) -> Option<Rect> {
    if !app.layout_prefix_pending() || !matches!(app.mode, Mode::Normal) {
        return None;
    }
    let width = screen.width.saturating_sub(4).min(70);
    let height = screen.height.saturating_sub(4).min(11);
    Some(Rect::new(
        screen.right().saturating_sub(width + 2),
        screen.bottom().saturating_sub(height + 2),
        width,
        height,
    ))
}
