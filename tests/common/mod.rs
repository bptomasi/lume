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
