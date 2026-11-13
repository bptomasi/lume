//! Read-only accounting contracts shared by SQLite and disk diagnostics.
//! Formatting and filesystem inspection belong to the disk usage service.

use serde::Serialize;

#[derive(Default)]
pub(crate) struct Usage {
    pub(crate) articles: u64,
    pub(crate) feed: u64,
    pub(crate) full: u64,
    pub(crate) notes: u64,
    pub(crate) comments: u64,
    pub(crate) highlights: u64,
    pub(crate) pages: u64,
    pub(crate) free: u64,
}

/// Logical publisher/cache bytes owned by one subscription, including archives.
#[derive(Debug, Serialize)]
pub struct SourceUsage {
    /// Stable local feed identity.
    pub feed_id: i64,
    /// Subscription label.
    pub title: String,
    /// Remote reader root, empty for ordinary feeds.
    pub server: String,
    /// All cached article rows, including Trash.
    pub articles: u64,
    /// Original publisher text bytes.
    pub feed_bytes: u64,
    /// Independently cached Full body bytes.
    pub full_bytes: u64,
}
/// Allocated pages belonging to a table/index/shadow table.
#[derive(Debug, Serialize)]
pub struct TableUsage {
    /// SQLite-owned object name.
    pub name: String,
    /// Physical allocated bytes, excluding WAL and free pages.
    pub allocated_bytes: u64,
}
/// Detailed, read-only disk accounting; compression is never enabled by inspection.
#[derive(Debug, Serialize)]
pub struct Details {
    /// Logical source accounting.
    pub sources: Vec<SourceUsage>,
    /// Physical object accounting from the bundled SQLite dbstat extension.
    pub tables: Vec<TableUsage>,
    /// Feed/Full body bytes (including separator) in the internal-content FTS table.
    pub fts_body_bytes: u64,
    /// Legacy note bytes retained in the article FTS table, if any.
    pub fts_note_bytes: u64,
    /// Observed body amplification and physical FTS page breakdown.
    pub duplication: Duplication,
}

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

impl Details {
    pub(crate) fn from_usage(
        sources: Vec<SourceUsage>,
        tables: Vec<TableUsage>,
        fts_body_bytes: u64,
        fts_note_bytes: u64,
    ) -> Self {
        let duplication =
            Duplication::measure(&sources, &tables, fts_body_bytes);
        Self {
            sources,
            tables,
            fts_body_bytes,
            fts_note_bytes,
            duplication,
        }
    }
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
}
