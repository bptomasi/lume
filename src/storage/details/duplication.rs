//! Observed byte amplification separates FTS content copies from index pages.
use super::{SourceUsage, TableUsage, Unit};
use serde::Serialize;

/// Measured logical amplification and allocated article-search pages.
#[derive(Debug, Serialize)]
pub struct Duplication {
    /// Sum of Feed and Full bodies in their owning tables.
    pub publisher_body_bytes: u64,
    /// Ratio of FTS body bytes to publisher body bytes; absent for empty text.
    pub fts_body_ratio: Option<f64>,
    /// Allocated pages of the FTS internal content table.
    pub fts_content_allocated_bytes: u64,
    /// Other FTS shadow pages, which include necessary search structures.
    pub fts_other_allocated_bytes: u64,
}

impl Duplication {
    pub(crate) fn measure(
        sources: &[SourceUsage],
        tables: &[TableUsage],
        fts: u64,
    ) -> Self {
        let publisher_body_bytes =
            sources.iter().fold(0u64, |total, source| {
                total
                    .saturating_add(source.feed_bytes)
                    .saturating_add(source.full_bytes)
            });
        let mut result = Self {
            publisher_body_bytes,
            fts_body_ratio: (publisher_body_bytes > 0)
                .then(|| fts as f64 / publisher_body_bytes as f64),
            fts_content_allocated_bytes: 0,
            fts_other_allocated_bytes: 0,
        };
        for table in tables {
            if table.name == "article_search_content" {
                result.fts_content_allocated_bytes = table.allocated_bytes;
            } else if table.name.starts_with("article_search_") {
                result.fts_other_allocated_bytes = result
                    .fts_other_allocated_bytes
                    .saturating_add(table.allocated_bytes);
            }
        }
        result
    }

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
