//! Row windows are shared by pointer hit tests and their renderers.

use ratatui::layout::Rect;

use crate::app::App;

/// Navigation identity under a screen row, excluding group labels and borders.
pub fn navigation_at(app: &App, area: Rect, row: u16) -> Option<usize> {
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    let rows = super::panes::navigation_rows(app, inner.width);
    let selected = rows
        .iter()
        .position(|(_, index)| *index == Some(app.nav_selected))?;
    let first = selected.saturating_sub(inner.height as usize / 2);
    rows.get(first + usize::from(row - inner.y))?.1
}

pub(super) fn article_window(app: &App, height: u16) -> (usize, usize, u16) {
    let row_height = if app.workspace.compact { 2 } else { 4 };
    let capacity = usize::from(height / row_height).max(1);
    let first = app
        .selected
        .saturating_sub(capacity / 2)
        .min(app.visible.len().saturating_sub(capacity));
    (first, capacity, row_height)
}

/// Article-page position under a row; unused trailing space selects nothing.
pub fn article_at(app: &App, area: Rect, row: u16) -> Option<usize> {
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    let (first, capacity, height) = article_window(app, inner.height);
    let local = usize::from((row - inner.y) / height);
    let position = first + local;
    (local < capacity && position < app.visible.len()).then_some(position)
}

pub(super) fn annotation_window(app: &App, height: u16) -> (usize, usize) {
    let capacity = usize::from(height / 6).max(1);
    let selected = app.annotation_selected().unwrap_or(0);
    let first = selected
        .saturating_sub(capacity / 2)
        .min(app.annotations.len().saturating_sub(capacity));
    (first, capacity)
}

/// Sorted annotation position under a card, excluding the panel border.
pub fn annotation_at(app: &App, area: Rect, row: u16) -> Option<usize> {
    let inner = area.inner(ratatui::layout::Margin::new(1, 1));
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    let (first, capacity) = annotation_window(app, inner.height);
    let local = usize::from((row - inner.y) / 6);
    let position = first + local;
    (local < capacity && position < app.annotations.len()).then_some(position)
}
