//! Physical files and logical text are separate views of the same library.

use super::Unit;

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

pub(super) fn text(usage: &Usage, sizes: [u64; 5], unit: Unit) -> String {
    let total = sizes
        .iter()
        .fold(0u64, |sum, size| sum.saturating_add(*size));
    let [database, wal, shared, ordinary, pinned] = sizes;
    let mut text = format!(
        "STORAGE\n\nTotal library + image files: {}\nArticles: {}\nNotes: {}   Highlights: {}\n\nFILES ON DISK\n",
        unit.format(total),
        usage.articles,
        usage.comments,
        usage.highlights,
    );
    for (name, size) in [
        ("Database file", database),
        ("WAL file", wal),
        ("SQLite shared memory", shared),
        ("Image cache", ordinary),
        ("Protected offline images", pinned),
    ] {
        text.push_str(&format!("{name:<26} {:>16}\n", unit.format(size)));
    }
    text.push_str("\nLOGICAL CONTENT (already included in database/WAL)\n");
    for (name, size) in [
        ("Feed text", usage.feed),
        ("Full text cache", usage.full),
        ("Notes text", usage.notes),
        ("SQLite allocated pages", usage.pages),
        ("Reusable database pages", usage.free),
    ] {
        text.push_str(&format!("{name:<26} {:>16}\n", unit.format(size)));
    }
    text.push_str(
        "\nUnits: :storage auto|MiB|GiB|bytes\n\
        :storage clean images|articles|all previews cleanup.\n\
        SQLite reuses freed pages; its file may retain its size.\n\
        Totals exclude backups/configuration; free pages are a subset.\n",
    );
    text
}
