//! A static, native-cell lighthouse: no image protocol, I/O or animation timer.
//!
//! Two vertical pixels share each terminal cell. The authored sprite scales down
//! to fit; compact terminals retain the wordmark and entry hint. Its background
//! follows the workspace theme while the stone and lantern keep their colors.

use ratatui::{
    Frame,
    buffer::Buffer,
    layout::{Alignment, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Paragraph},
};

use super::theme;

const WIDTH: u16 = 80;
const HEIGHT: u16 = 46;
const LIGHTHOUSE: &[u8] = include_bytes!("welcome/lighthouse.txt");

/// Draw the entry screen without accessing or changing library state.
pub fn draw(frame: &mut Frame<'_>, name: &str) {
    let area = frame.area();
    frame
        .render_widget(Block::new().style(Style::new().bg(theme::PANEL)), area);
    let art = art_area(area);
    let height = if art.height == 0 { 5 } else { art.height + 7 };
    let top = area.y + area.height.saturating_sub(height) / 2;
    let title = top + if art.height == 0 { 0 } else { art.height + 1 };
    caption(frame, title, "l u m e", theme::ACCENT, true);
    if area.width >= 36 && area.height >= 7 {
        caption(
            frame,
            title + 1,
            "a quiet place for curious minds",
            theme::MUTED,
            false,
        );
    }
    let hint = if area.width >= 28 {
        "Press any key to begin"
    } else {
        "Press any key"
    };
    caption(frame, title + 3, hint, theme::FAINT, false);
    theme::apply(frame.buffer_mut(), name);
    paint(frame.buffer_mut(), art, name);
}

fn art_area(area: Rect) -> Rect {
    if area.width < 36 || area.height < 18 {
        return Rect::default();
    }
    let rows = area.height.saturating_sub(9).min(HEIGHT / 2);
    let width = area.width.saturating_sub(4).min(rows * 2 * WIDTH / HEIGHT);
    let height = width * HEIGHT / WIDTH / 2;
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + area.height.saturating_sub(height + 7) / 2,
        width,
        height,
    )
}

fn caption(
    frame: &mut Frame<'_>,
    y: u16,
    text: &str,
    color: Color,
    bold: bool,
) {
    let area = frame.area();
    if y >= area.bottom() {
        return;
    }
    let style = Style::new().fg(color);
    let style = if bold {
        style.add_modifier(Modifier::BOLD)
    } else {
        style
    };
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(style),
        Rect::new(area.x, y, area.width, 1),
    );
}

fn paint(buffer: &mut Buffer, area: Rect, name: &str) {
    for row in 0..area.height {
        for column in 0..area.width {
            let x = column * WIDTH / area.width;
            let top = row * 2 * HEIGHT / (area.height * 2);
            let bottom = (row * 2 + 1) * HEIGHT / (area.height * 2);
            let foreground = pixel(x, top, name);
            let background = pixel(x, bottom, name);
            let symbol = if foreground == background { " " } else { "▀" };
            buffer[(area.x + column, area.y + row)]
                .set_symbol(symbol)
                .set_fg(foreground)
                .set_bg(background);
        }
    }
}

fn pixel(x: u16, y: u16, name: &str) -> Color {
    let index = usize::from(y) * usize::from(WIDTH + 1) + usize::from(x);
    let [red, green, blue] = match LIGHTHOUSE.get(index) {
        Some(b'a') => [36, 33, 30],
        Some(b'b') => [51, 44, 35],
        Some(b'c') => [102, 82, 57],
        Some(b'd') => [225, 179, 109],
        Some(b'e') => [242, 212, 157],
        Some(b'w') => [224, 219, 208],
        Some(b's') => [55, 55, 57],
        Some(b'm') => [114, 113, 110],
        Some(b'l') => [170, 167, 159],
        Some(b'o') => [38, 38, 41],
        _ => theme::image_background(name),
    };
    Color::Rgb(red, green, blue)
}

#[cfg(test)]
mod tests;
