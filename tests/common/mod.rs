//! Small scenario setup shared by native integration tests.

use anyhow::{Result, anyhow};
use lume::{model::Feed, source, store::Store};

pub fn seeded_store() -> Result<(tempfile::TempDir, Store, Feed)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let id = store.subscribe("https://journal.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .ok_or_else(|| anyhow!("Missing scenario subscription"))?;
    let parsed =
        source::parse(include_bytes!("../fixtures/rss.xml"), &feed.url)?;
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    Ok((directory, store, feed))
}

/// Settle annotation workers with a deadline, exercising their real poll path.
#[allow(
    dead_code,
    reason = "Each integration target selects its own scenarios"
)]
pub fn settle_notes(app: &mut lume::app::App) -> Result<()> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        app.poll_reader();
        app.poll_navigation()?;
        app.poll_note_workspace()?;
        if !app.navigation_loading
            && !app.notes_pending()
            && !app.notebook_pending()
            && !app.note_inspection_pending()
            && !app.note_catalog_pending()
        {
            return Ok(());
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Note worker timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
