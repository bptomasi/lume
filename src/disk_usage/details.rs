//! Explicit diagnostics distinguish logical source bytes from physical SQLite pages.
mod duplication;

use anyhow::Result;

use super::Unit;
pub use crate::model::disk_usage::{
    Details, Duplication, SourceUsage, TableUsage,
};
use crate::store::Store;

/// Measure all source/object diagnostics in one WAL snapshot.
///
/// # Errors
/// Returns SQLite/query errors without modifying data.
pub fn inspect(store: &Store) -> Result<Details> {
    store.storage_details()
}

impl Details {
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
