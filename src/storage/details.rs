//! Explicit diagnostics distinguish logical source bytes from physical SQLite pages.
use super::Unit;
mod duplication;
use crate::store::Store;
use anyhow::Result;
pub use duplication::Duplication;
use serde::Serialize;

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

/// Measure all source/object diagnostics in one WAL snapshot.
///
/// # Errors
/// Returns SQLite/query errors without modifying data.
pub fn inspect(store: &Store) -> Result<Details> {
    store.storage_details()
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

    /// Render exact accounting in the selected binary display unit.
    pub fn text(&self, unit: Unit) -> String {
        let mut text =
            String::from("BY SOURCE · logical bytes, including Trash\n");
        for source in &self.sources {
            text.push_str(&format!(
                "{} · {} articles · Feed {} · Full {}\n",
                crate::content::label(&source.title, 60),
                source.articles,
                unit.format(source.feed_bytes),
                unit.format(source.full_bytes)
            ));
            if !source.server.is_empty() {
                text.push_str(&format!("  Server: {}\n", source.server));
            }
        }
        text.push_str(
            "\nSQLITE OBJECTS · allocated pages, excludes WAL/free pages\n",
        );
        for table in &self.tables {
            text.push_str(&format!(
                "{:<42} {}\n",
                table.name,
                unit.format(table.allocated_bytes)
            ));
        }
        text.push_str(&format!("\nFTS INTERNAL CONTENT COPIES\nSearchable bodies: {}\nLegacy notes: {}\n\nFTS index pages also contain necessary search data.\nThese logical copies are not additional filesystem totals.\nCompression remains disabled.\n",
            unit.format(self.fts_body_bytes), unit.format(self.fts_note_bytes)));
        text.push_str(&self.duplication.text(unit));
        text
    }
}
