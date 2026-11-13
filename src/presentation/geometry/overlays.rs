//! Bounded popup and linked-reading rectangles shared by input and paint.

use ratatui::layout::{Constraint, Layout, Position, Rect};

/// One picker footprint shared by its frame, previews and pointer input.
pub fn picker_area(screen: Rect) -> Rect {
    centered(screen, 110, 28)
}

/// Choice and preview rectangles shared by selector drawing and mouse input.
pub fn picker_regions(screen: Rect) -> (Rect, Rect) {
    let outer = picker_area(screen);
    let inner = outer.inner(ratatui::layout::Margin::new(1, 1));
    let body = Rect::new(
        inner.x,
        inner.y + 2,
        inner.width,
        inner.height.saturating_sub(4),
    );
    let panes = if body.width >= 80 {
        Layout::horizontal([
            Constraint::Percentage(52),
            Constraint::Percentage(48),
        ])
        .split(body)
    } else {
        Layout::vertical([
            Constraint::Percentage(65),
            Constraint::Percentage(35),
        ])
        .split(body)
    };
    (panes[0], panes[1])
}

/// Centered linked-reading column shared by rendering and pointer input.
pub fn web_content(screen: Rect, width: usize) -> Rect {
    let width = (width as u16).min(screen.width.saturating_sub(4));
    Rect::new(
        screen.x + screen.width.saturating_sub(width) / 2,
        screen.y + screen.height.min(3),
        width,
        screen.height.saturating_sub(6),
    )
}

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

/// Popup rectangle shared by drawing and hit testing, clamped to the terminal.
pub fn note_preview_area(screen: Rect, anchor: Position) -> Rect {
    let width = screen.width.saturating_sub(2).min(56);
    let height = screen.height.saturating_sub(2).min(9);
    let x = anchor
        .x
        .saturating_add(2)
        .min(screen.right().saturating_sub(width + 1))
        .max(screen.x);
    let y = if anchor.y.saturating_add(height + 1) < screen.bottom() {
        anchor.y + 1
    } else {
        anchor.y.saturating_sub(height)
    };
    Rect::new(
        x,
        y.clamp(screen.y, screen.bottom().saturating_sub(height)),
        width,
        height,
    )
}

/// Center a bounded popup while preserving a one-cell outer margin.
pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}
