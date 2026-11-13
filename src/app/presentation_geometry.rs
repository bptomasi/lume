//! Application maps editor and annotation identities to neutral hit contracts.

use super::{App, InputKind, Mode};
use crate::{model::Pane, presentation::hit::NavigationHits};

impl App {
    /// Borrow cached row identities and inline-search occupancy without work.
    pub fn navigation_hits(&self) -> NavigationHits<'_> {
        let navigation = self.navigation();
        let input = match self.mode() {
            Mode::Input(input) => Some(&input.kind),
            _ => None,
        };
        NavigationHits {
            navigation,
            compact: self.workspace().compact,
            library_search: !navigation.library_query.is_empty()
                || matches!(input, Some(InputKind::LibrarySearch)),
            article_search: !navigation.search.is_empty()
                || matches!(input, Some(InputKind::Search)),
        }
    }

    pub(super) fn navigation_origin(&self) -> Option<(usize, usize)> {
        let layout = self.layout();
        let area = crate::presentation::geometry::workspace_areas(
            layout.screen,
            layout.focus,
            layout.workspace,
        )
        .get(Pane::Library)?;
        self.navigation_hits().navigation_origin(area)
    }

    pub(super) fn annotation_hit(
        &self,
        area: ratatui::layout::Rect,
        row: u16,
    ) -> Option<usize> {
        let reading = self.reader();
        crate::presentation::hit::annotation_at(
            reading.annotations.len(),
            reading.annotation_selected().unwrap_or(0),
            area,
            row,
        )
    }
}
