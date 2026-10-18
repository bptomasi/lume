//! Oversized panes use an opaque, bounded summary instead of a clipped wall.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Widget},
};

use crate::model::Pane;
use unicode_width::UnicodeWidthStr;

pub(crate) fn compact(frame: &Buffer, origin: Rect, pane: Pane) -> Buffer {
    let area = Rect::new(
        origin.x,
        origin.y,
        origin.width.min(38),
        origin.height.min(9),
    );
    let mut picture = Buffer::empty(area);
    let color = &frame[(origin.x, origin.y)];
    let block = Block::bordered()
        .border_type(BorderType::Rounded)
        .title(format!(" {} · moving ", pane.title()))
        .style(Style::new().fg(color.fg).bg(color.bg));
    block.render(area, &mut picture);
    for row in 1..area.height.saturating_sub(1) {
        for column in 1..area.width.saturating_sub(1) {
            let cell = &mut picture[(area.x + column, area.y + row)];
            *cell = frame[(origin.x + column, origin.y + row)].clone();
            // Keep a wide grapheme from consuming the summary's right frame.
            if cell.symbol().width() > usize::from(area.width - 1 - column) {
                cell.set_symbol(" ");
            }
        }
    }
    picture
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summary_crops_wide_text_before_its_own_border_without_touching_source() {
        let origin = Rect::new(10, 2, 100, 20);
        let mut source = Buffer::empty(origin);
        source[(44, 3)].set_symbol("界");
        source[(46, 3)].set_symbol("界");

        let picture = compact(&source, origin, Pane::Reader);

        assert_eq!(picture[(44, 3)].symbol(), "界");
        assert_eq!(picture[(46, 3)].symbol(), " ");
        assert_eq!(picture[(47, 3)].symbol(), "│");
        assert_eq!(source[(46, 3)].symbol(), "界");
    }
}
