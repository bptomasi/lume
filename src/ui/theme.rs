//! Fixed sRGB palette; no font patches or terminal theme assumptions required.

use ratatui::{
    style::{Color, Style},
    widgets::{Block, BorderType},
};

pub(super) const BG: Color = Color::Rgb(18, 18, 20);
pub(super) const PANEL: Color = Color::Rgb(23, 23, 26);
pub(super) const RAISED: Color = Color::Rgb(37, 35, 33);
pub(super) const BORDER: Color = Color::Rgb(54, 52, 51);
pub(super) const TEXT: Color = Color::Rgb(224, 219, 208);
pub(super) const MUTED: Color = Color::Rgb(155, 151, 144);
pub(super) const FAINT: Color = Color::Rgb(121, 120, 117);
pub(super) const ACCENT: Color = Color::Rgb(225, 179, 109);
pub(super) const AMBER: Color = Color::Rgb(225, 179, 109);
pub(super) const BLUE: Color = Color::Rgb(145, 180, 218);

pub(crate) fn image_background(name: &str) -> [u8; 3] {
    match name {
        "nord" => [52, 60, 75],
        "gruvbox" => [40, 40, 40],
        _ => [23, 23, 26],
    }
}

pub(super) fn panel(title: String, focused: bool) -> Block<'static> {
    let color = if focused { ACCENT } else { BORDER };
    Block::bordered()
        .border_type(BorderType::Rounded)
        .title(title)
        .title_style(Style::new().fg(if focused { ACCENT } else { MUTED }))
        .border_style(Style::new().fg(color))
        .style(Style::new().bg(PANEL).fg(TEXT))
}

pub(super) fn apply(buffer: &mut ratatui::buffer::Buffer, name: &str) {
    let colors = match name {
        "nord" => [
            0x2e3440, 0x343c4b, 0x434c5e, 0x4c566a, 0xeceff4, 0xd8dee9,
            0x9daabd, 0x88c0d0, 0x81a1c1,
        ],
        "gruvbox" => [
            0x1d2021, 0x282828, 0x3c3836, 0x504945, 0xebdbb2, 0xbdae93,
            0x928374, 0xfabd2f, 0x83a598,
        ],
        _ => return,
    };
    let source = [BG, PANEL, RAISED, BORDER, TEXT, MUTED, FAINT, AMBER, BLUE];
    for cell in &mut buffer.content {
        // Raster cells retain publisher colors, even when a pixel happens to
        // equal a UI palette token. Transparency already uses the chosen theme.
        if matches!(cell.symbol(), "▀" | "▄") {
            continue;
        }
        if let Some(index) = source.iter().position(|color| *color == cell.fg) {
            cell.set_fg(rgb(colors[index]));
        }
        if let Some(index) = source.iter().position(|color| *color == cell.bg) {
            cell.set_bg(rgb(colors[index]));
        }
    }
}

fn rgb(value: u32) -> Color {
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}
