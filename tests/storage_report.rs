//! Binary units, physical totals and large galleries qualify real storage paths.
mod common;

use anyhow::Result;
use lume::{
    media::cache,
    storage::{self, Cleanup, Unit},
};
use std::sync::atomic::AtomicBool;

#[test]
fn binary_units_are_explicit_and_keep_small_nonzero_sizes_visible() {
    assert_eq!(Unit::Auto.format(0), "0 B");
    assert_eq!(Unit::Auto.format(1023), "1023 B");
    assert_eq!(Unit::Auto.format(1024), "1.00 KiB");
    assert_eq!(Unit::Auto.format(1 << 20), "1.00 MiB");
    assert_eq!(Unit::MiB.format(3 << 19), "1.50 MiB");
    assert_eq!(Unit::MiB.format(1), "<0.01 MiB");
    assert_eq!(Unit::GiB.format(1 << 30), "1.00 GiB");
    assert_eq!(Unit::Bytes.format(u64::MAX), format!("{} B", u64::MAX));
    assert_eq!("mIb".parse::<Unit>().ok(), Some(Unit::MiB));
    assert!("MB".parse::<Unit>().is_err());
}

#[test]
fn report_separates_physical_totals_and_logical_content_in_all_units()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    let images = root.join("images");
    cache::write(&images, "https://images.example/ordinary", b"ordinary")?;
    cache::write(
        &images.join("offline"),
        "https://images.example/pin",
        b"pin",
    )?;
    let physical = [
        "library.sqlite3",
        "library.sqlite3-wal",
        "library.sqlite3-shm",
    ]
    .iter()
    .try_fold(11u64, |total, name| -> Result<u64> {
        Ok(total + std::fs::metadata(root.join(name))?.len())
    })?;
    for unit in [Unit::Auto, Unit::MiB, Unit::GiB, Unit::Bytes] {
        let report = storage::inspect_with_unit(&store, &images, unit)?;
        assert!(report.contains(&format!(
            "Total library + image files: {}",
            unit.format(physical)
        )));
        assert!(report.contains("Articles: 1"));
        assert!(report.contains("FILES ON DISK"));
        assert!(report.contains("LOGICAL CONTENT"));
        assert!(report.contains("Protected offline images"));
    }
    Ok(())
}

#[test]
fn manual_image_cleanup_handles_long_galleries_and_protects_pinned_files()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    let body = (0..80)
        .map(|id| format!("![{id}](https://images.example/{id})\n\n"))
        .collect::<String>();
    parsed.articles[0].body = body.clone();
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    let images = directory.path().join("images");
    let kept = "https://images.example/79";
    let orphan = "https://images.example/orphan";
    cache::write(&images, kept, b"kept")?;
    cache::write(&images, orphan, b"orphan")?;
    cache::write(&images.join("offline"), orphan, b"pinned")?;
    let report = storage::clean(
        &store,
        &images,
        Cleanup::Images,
        &AtomicBool::new(false),
    )?;
    assert!(report.contains("Removed 1 orphaned"));
    assert!(cache::path(&images, kept).exists());
    assert!(!cache::path(&images, orphan).exists());
    assert!(cache::path(&images.join("offline"), orphan).exists());
    assert_eq!(store.body(store.library()?.articles[0].id)?, body);
    Ok(())
}

#[test]
fn detailed_accounting_measures_utf8_fts_and_archived_sources_without_writes()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = "á漢字 text".into();
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    store.set_starred(store.library()?.articles[0].id, true)?;
    store.unsubscribe(feed.id)?;
    let details = storage::details::inspect(&store)?;
    assert_eq!(details.sources.len(), 1);
    assert_eq!(details.sources[0].articles, 1);
    assert_eq!(details.sources[0].feed_bytes, "á漢字 text".len() as u64);
    assert_eq!(details.fts_body_bytes, details.sources[0].feed_bytes + 1);
    assert_eq!(
        details.duplication.publisher_body_bytes,
        details.sources[0].feed_bytes
    );
    assert!(
        details
            .duplication
            .fts_body_ratio
            .is_some_and(|ratio| ratio > 1.0)
    );
    assert!(details.duplication.fts_content_allocated_bytes > 0);
    assert!(details.duplication.fts_other_allocated_bytes > 0);
    assert!(
        details
            .tables
            .iter()
            .any(|table| table.name == "articles" && table.allocated_bytes > 0)
    );
    assert!(
        details
            .text(Unit::MiB)
            .contains("FTS INTERNAL CONTENT COPIES")
    );
    let json = serde_json::to_value(&details)?;
    assert_eq!(json["sources"][0]["feed_id"], feed.id);
    assert_eq!(store.body(store.library()?.articles[0].id)?, "á漢字 text");
    Ok(())
}

#[test]
fn empty_storage_reports_no_body_ratio_without_inventing_a_saving() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store =
        lume::store::Store::open(&directory.path().join("empty.sqlite3"))?;
    let details = storage::details::inspect(&store)?;
    assert_eq!(details.duplication.publisher_body_bytes, 0);
    assert!(details.duplication.fts_body_ratio.is_none());
    assert!(
        details
            .text(Unit::Bytes)
            .contains("n/a (empty publisher text)")
    );
    Ok(())
}
