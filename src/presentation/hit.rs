//! Shared visible-row windows for cached painting and pointer hit tests.

use super::navigation::{NavigationView, Row, window};
use ratatui::layout::{Margin, Rect};

/// Search fields reserve at most two rows, including their control hint.
pub fn search_content(inner: Rect, searching: bool) -> Rect {
    let rows = if searching { inner.height.min(2) } else { 0 };
    Rect::new(inner.x, inner.y + rows, inner.width, inner.height - rows)
}

/// Navigation row identities including fold controls and excluding headings.
pub fn navigation_at(
    navigation: NavigationView<'_>,
    searching: bool,
    area: Rect,
    row: u16,
) -> Option<usize> {
    let inner = search_content(area.inner(Margin::new(1, 1)), searching);
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    navigation.navigation.get(navigation.nav_selected)?;
    match window(navigation, inner.height).nth(usize::from(row - inner.y))? {
        Row::Item(index) => Some(index),
        Row::Blank | Row::Heading(_) => None,
    }
}

/// Stable article window, shifting only enough to reveal the selection.
pub fn article_window(
    count: usize,
    selected: usize,
    pinned: usize,
    compact: bool,
    height: u16,
) -> (usize, usize, u16) {
    let row_height = if compact { 2 } else { 4 };
    let capacity = usize::from(height / row_height).max(1);
    let last = count.saturating_sub(capacity);
    let selected = selected.min(count.saturating_sub(1));
    let first = pinned
        .min(last)
        .min(selected)
        .max(selected.saturating_sub(capacity - 1).min(last));
    (first, capacity, row_height)
}

/// Article-page identity under a painted card; trailing space is inactive.
fn article_at(
    window: (usize, usize, u16),
    count: usize,
    inner: Rect,
    row: u16,
) -> Option<usize> {
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    let (first, capacity, height) = window;
    let local = usize::from((row - inner.y) / height);
    let position = first + local;
    (local < capacity && position < count).then_some(position)
}

/// Center the selected annotation within bounded six-row cards.
pub fn annotation_window(
    count: usize,
    selected: usize,
    height: u16,
) -> (usize, usize) {
    let capacity = usize::from(height / 6).max(1);
    let first = selected
        .saturating_sub(capacity / 2)
        .min(count.saturating_sub(capacity));
    (first, capacity)
}

/// Cached navigation metadata and search-row occupancy for paint and input.
#[derive(Clone, Copy)]
pub struct NavigationHits<'a> {
    /// Identity-stable cached article and sidebar metadata.
    pub navigation: NavigationView<'a>,
    /// Two-row article cards instead of four-row cards.
    pub compact: bool,
    /// Library query or active inline editor reserves two rows.
    pub library_search: bool,
    /// Article query or active inline editor reserves two rows.
    pub article_search: bool,
}

impl NavigationHits<'_> {
    /// Sidebar identity under a painted row, including folded sections.
    pub fn navigation_at(self, area: Rect, row: u16) -> Option<usize> {
        navigation_at(self.navigation, self.library_search, area, row)
    }

    /// Article-page position under a painted card.
    pub fn article_at(self, area: Rect, row: u16) -> Option<usize> {
        article_at(
            self.article_window(area),
            self.navigation.visible.len(),
            self.article_content(area),
            row,
        )
    }

    /// Article content rectangle after borders and inline search controls.
    pub fn article_content(self, area: Rect) -> Rect {
        search_content(area.inner(Margin::new(1, 1)), self.article_search)
    }

    /// Article window in the exact rectangle used to paint cards.
    pub fn article_window(self, area: Rect) -> (usize, usize, u16) {
        let navigation = self.navigation;
        article_window(
            navigation.visible.len(),
            navigation.selected,
            navigation.articles_first,
            self.compact,
            self.article_content(area).height,
        )
    }

    /// First article position pinned when pointer or resize changes geometry.
    pub fn article_origin(self, area: Rect) -> usize {
        self.article_window(area).0
    }

    /// Sidebar origin including any skipped heading rows.
    pub fn navigation_origin(self, area: Rect) -> Option<(usize, usize)> {
        let inner =
            search_content(area.inner(Margin::new(1, 1)), self.library_search);
        (inner.height > 0).then(|| {
            super::navigation::window_start(self.navigation, inner.height)
        })
    }
}

/// Navigation identity under a painted row, including folded-section controls.
pub fn navigation_hit(
    hits: NavigationHits<'_>,
    area: Rect,
    row: u16,
) -> Option<usize> {
    hits.navigation_at(area, row)
}

/// Article-page position under a painted card.
pub fn article_hit(
    hits: NavigationHits<'_>,
    area: Rect,
    row: u16,
) -> Option<usize> {
    hits.article_at(area, row)
}

/// Annotation position under a six-row card, excluding the panel border.
pub fn annotation_at(
    count: usize,
    selected: usize,
    area: Rect,
    row: u16,
) -> Option<usize> {
    let inner = area.inner(Margin::new(1, 1));
    if row < inner.y || row >= inner.bottom() {
        return None;
    }
    let (first, capacity) = annotation_window(count, selected, inner.height);
    let local = usize::from((row - inner.y) / 6);
    let position = first + local;
    (local < capacity && position < count).then_some(position)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SEARCH_CARD_HITS: &[(bool, u16, &[Option<usize>])] = &[
        (false, 3, &[None, None, None]),
        (false, 5, &[None, None, None, Some(0), None]),
        (
            false,
            12,
            &[
                None,
                None,
                None,
                Some(0),
                Some(0),
                Some(0),
                Some(0),
                Some(1),
                Some(1),
                Some(1),
                Some(1),
                None,
            ],
        ),
        (
            true,
            11,
            &[
                None,
                None,
                None,
                Some(0),
                Some(0),
                Some(1),
                Some(1),
                Some(2),
                Some(2),
                None,
                None,
            ],
        ),
    ];

    #[test]
    fn search_controls_and_partial_cards_share_hit_rows_at_small_heights() {
        for &(compact, height, expected) in SEARCH_CARD_HITS {
            let pane = Rect::new(7, 11, 30, height);
            let inner = search_content(pane.inner(Margin::new(1, 1)), true);
            let window = article_window(3, 0, 0, compact, inner.height);
            let actual = (pane.y..pane.bottom())
                .map(|row| article_at(window, 3, inner, row))
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "{compact} · {height}");
        }
    }

    #[test]
    fn annotation_borders_and_unused_card_rows_are_inactive() {
        let area = Rect::new(7, 11, 30, 24);
        assert_eq!(annotation_at(1, 0, area, area.y), None);
        assert_eq!(annotation_at(1, 0, area, area.y + 1), Some(0));
        assert_eq!(annotation_at(1, 0, area, area.y + 6), Some(0));
        assert_eq!(annotation_at(1, 0, area, area.y + 7), None);
        assert_eq!(annotation_at(1, 0, area, area.bottom() - 1), None);
        assert_eq!(annotation_at(1, 0, Rect::default(), 0), None);
    }
}
