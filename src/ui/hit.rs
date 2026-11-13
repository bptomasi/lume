//! Pane renderers share neutral windows with the pointer controller.

use crate::app::{NavigationPaneView, ReaderView};

pub(super) fn article_window(
    view: &NavigationPaneView<'_>,
    height: u16,
) -> (usize, usize, u16) {
    let nav = view.navigation;
    crate::presentation::hit::article_window(
        nav.visible.len(),
        nav.selected,
        nav.articles_first,
        view.compact,
        height,
    )
}

pub(super) fn annotation_window(
    reader: ReaderView<'_>,
    height: u16,
) -> (usize, usize) {
    crate::presentation::hit::annotation_window(
        reader.annotations.len(),
        reader.annotation_selected().unwrap_or(0),
        height,
    )
}
