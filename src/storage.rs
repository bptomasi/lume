//! User-requested storage inspection and conservative recoverable-cache cleanup.

use std::{
    collections::BTreeSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use anyhow::{Result, ensure};

use crate::store::Store;

/// Recoverable cache scope; no action deletes articles or personal records.
#[derive(Clone, Copy, Debug)]
pub enum Cleanup {
    /// Orphaned ordinary images only; pinned offline images are protected.
    Images,
    /// Unprotected Full article bodies only.
    Articles,
    /// Full bodies followed by orphaned images.
    All,
}

/// Inspect SQLite logical usage and actual image/WAL files on a worker.
///
/// # Errors
/// Returns storage or filesystem errors.
pub fn inspect(store: &Store, images: &Path) -> Result<String> {
    let mut report = store.storage_report()?;
    let path = store.database_path()?;
    let wal = std::path::PathBuf::from(format!("{}-wal", path.display()));
    let wal_bytes = match std::fs::metadata(wal) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    };
    let ordinary = files(images)?.iter().map(|entry| entry.1).sum::<u64>();
    let pinned = files(&images.join("offline"))?
        .iter()
        .map(|entry| entry.1)
        .sum::<u64>();
    report.push_str(&format!(
        r#"WAL file: {wal_bytes} bytes
Image cache: {ordinary} bytes
Protected offline images: {pinned} bytes
"#
    ));
    report.push_str(
        r#"
:storage clean images|articles|all previews an explicit cleanup.
Feed text, articles and personal data are never removed.
SQLite reuses freed pages; the database file may retain its size.
"#,
    );
    Ok(report)
}

/// Reclaim unprotected Full text in small transactions and orphaned images.
///
/// # Errors
/// Returns filesystem or SQLite failures. Completed batches remain committed;
/// a later failure cannot roll them back or affect protected personal data.
pub fn clean(
    store: &Store,
    images: &Path,
    scope: Cleanup,
    cancel: &AtomicBool,
) -> Result<String> {
    let mut articles = 0;
    let mut bytes = 0;
    if matches!(scope, Cleanup::Articles | Cleanup::All) {
        let upper = store.cleanup_boundary()?;
        let mut after = 0;
        while !cancel.load(Ordering::Relaxed) {
            let batch = store.clear_document_batch(after, upper)?;
            after = batch.2;
            articles += batch.0;
            bytes += batch.1;
            if batch.0 == 0 {
                break;
            }
            std::thread::yield_now();
        }
    }
    let images_removed = if matches!(scope, Cleanup::Images | Cleanup::All)
        && !cancel.load(Ordering::Relaxed)
    {
        clean_images(store, images, cancel)?
    } else {
        0
    };
    Ok(format!(
        r#"Cleared {articles} unprotected Full caches ({bytes} logical bytes).
Removed {images_removed} orphaned ordinary images.
Protected records and pinned images remain intact.
Cancelled: {}
"#,
        cancel.load(Ordering::Relaxed)
    ))
}

fn clean_images(
    store: &Store,
    directory: &Path,
    cancel: &AtomicBool,
) -> Result<usize> {
    let started = SystemTime::now();
    let mut retained = BTreeSet::new();
    store.visit_cached_bodies(|body| {
        for address in crate::offline::image_addresses(body)? {
            retained.insert(crate::media::cache::path(directory, &address));
            ensure!(
                retained.len() <= 100_000,
                "Image reference limit reached; no images were removed"
            );
        }
        Ok(())
    })?;
    let mut removed = 0;
    for (path, _, modified) in files(directory)? {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        if !retained.contains(&path) && modified < started {
            std::fs::remove_file(path)?;
            removed += 1;
        }
    }
    Ok(removed)
}

pub(crate) fn files(
    directory: &Path,
) -> Result<Vec<(std::path::PathBuf, u64, SystemTime)>> {
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error.into()),
    };
    let mut files = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && crate::media::cache::owned_filename(
                &entry.file_name().to_string_lossy(),
            )
        {
            let metadata = entry.metadata()?;
            files.push((entry.path(), metadata.len(), metadata.modified()?));
        }
    }
    Ok(files)
}
