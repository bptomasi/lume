//! Row windows are shared by pointer hit tests and their renderers.

use ratatui::layout::Rect;

use crate::app::App;

/// Navigation identity under a row, including section controls; excludes borders.
pub fn navigation_at(app: &App, area: Rect, row: u16) -> Option<usize> {
    let inner = super::pane_search::content(
        app,
        crate::model::Pane::Library,
        area.inner(ratatui::layout::Margin::new(1, 1)),
    );
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    super::navigation::at(app, inner.height, usize::from(row - inner.y))
}

pub(super) fn article_window(app: &App, height: u16) -> (usize, usize, u16) {
    let row_height = if app.workspace.compact { 2 } else { 4 };
    let capacity = usize::from(height / row_height).max(1);
    let last = app.visible.len().saturating_sub(capacity);
    let selected = app.selected.min(app.visible.len().saturating_sub(1));
    let first = app
        .articles_first
        .min(last)
        .min(selected)
        .max(selected.saturating_sub(capacity - 1).min(last));
    (first, capacity, row_height)
}

pub(crate) fn article_origin(app: &App, area: Rect) -> usize {
    let inner = super::pane_search::content(
        app,
        crate::model::Pane::Articles,
        area.inner(ratatui::layout::Margin::new(1, 1)),
    );
    article_window(app, inner.height).0
}

/// Article-page position under a row; unused trailing space selects nothing.
pub fn article_at(app: &App, area: Rect, row: u16) -> Option<usize> {
    let inner = super::pane_search::content(
        app,
        crate::model::Pane::Articles,
        area.inner(ratatui::layout::Margin::new(1, 1)),
    );
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
