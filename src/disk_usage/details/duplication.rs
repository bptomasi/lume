//! Observed byte amplification separates FTS content copies from index pages.
use super::{Duplication, Unit};

impl Duplication {
    pub(super) fn text(&self, unit: Unit) -> String {
        let ratio = self.fts_body_ratio.map_or_else(
            || "n/a (empty publisher text)".into(),
            |ratio| format!("{ratio:.3}×"),
        );
        format!(
            "\nMEASURED TEXT AMPLIFICATION\nPublisher Feed + Full bodies: {}\nFTS body bytes / publisher body bytes: {ratio}\nFTS content allocated pages: {}\nOther article FTS allocated pages: {}\nThe ratio includes FTS separators; page sizes also include titles and metadata.\nContent copies and search structures have different storage costs.\n",
            unit.format(self.publisher_body_bytes),
            unit.format(self.fts_content_allocated_bytes),
            unit.format(self.fts_other_allocated_bytes)
        )
    }
}
