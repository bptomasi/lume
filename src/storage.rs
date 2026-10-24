//! User-requested storage inspection and conservative recoverable-cache cleanup.

use std::{
    collections::BTreeSet,
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
    time::SystemTime,
};

use anyhow::{Result, ensure};

use crate::store::Store;

type ImageFile = (std::path::PathBuf, u64, SystemTime);

pub mod details;
pub(crate) mod report;
mod units;
pub use units::Unit;

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
    inspect_with_unit(store, images, Unit::Auto)
}

/// Inspect using a requested binary unit, with physical and logical usage apart.
///
/// # Errors
/// Returns storage or filesystem errors; inspection never reclaims content.
pub fn inspect_with_unit(
    store: &Store,
    images: &Path,
    unit: Unit,
) -> Result<String> {
    inspect_checked(store, images, unit, &|| Ok(()))
}

pub(crate) fn inspect_checked(
    store: &Store,
    images: &Path,
    unit: Unit,
    check: &dyn Fn() -> Result<()>,
) -> Result<String> {
    check()?;
    let usage = store.read_snapshot(Store::storage_usage)?;
    let path = store.database_path()?;
    let mut sizes = [0; 5];
    for (index, suffix) in ["", "-wal", "-shm"].into_iter().enumerate() {
        let mut file = path.as_os_str().to_os_string();
        file.push(suffix);
        sizes[index] = file_bytes(Path::new(&file))?;
    }
    sizes[3] = image_file_bytes(images, check)?;
    sizes[4] = image_file_bytes(&images.join("offline"), check)?;
    check()?;
    Ok(report::text(&usage, sizes, unit))
}

fn image_file_bytes(
    directory: &Path,
    check: &dyn Fn() -> Result<()>,
) -> Result<u64> {
    check()?;
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(0);
        }
        Err(error) => return Err(error.into()),
    };
    let mut bytes = 0u64;
    for entry in entries {
        check()?;
        let entry = entry?;
        if entry.file_type()?.is_file()
            && crate::media::cache::owned_filename(
                &entry.file_name().to_string_lossy(),
            )
        {
            bytes = bytes.saturating_add(entry.metadata()?.len());
        }
    }
    Ok(bytes)
}

fn file_bytes(path: &Path) -> Result<u64> {
    Ok(match std::fs::metadata(path) {
        Ok(metadata) => metadata.len(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => 0,
        Err(error) => return Err(error.into()),
    })
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
    let mut candidates = files(directory)?;
    if candidates.is_empty() {
        return Ok(0);
    }
    let cached: BTreeSet<_> =
        candidates.iter().map(|(path, _, _)| path.clone()).collect();
    let version = store.retention_version()?;
    if !store.refresh_image_references(cancel)? {
        return Ok(0);
    }
    let retained = retained_images(store, directory, &cached, cancel)?;
    candidates.retain(|(path, _, modified)| {
        !retained.contains(path) && *modified < started
    });
    remove_images(store, &candidates, version, cancel)
}

fn remove_images(
    store: &Store,
    candidates: &[ImageFile],
    version: i64,
    cancel: &AtomicBool,
) -> Result<usize> {
    let mut removed = 0;
    for batch in candidates.chunks(32) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        let count = store.image_cleanup_batch(version, || {
            let mut count = 0;
            for (path, bytes, modified) in batch {
                if cancel.load(Ordering::Relaxed) {
                    break;
                }
                count +=
                    usize::from(remove_unchanged(path, *bytes, *modified)?);
            }
            Ok(count)
        })?;
        let Some(count) = count else {
            break;
        };
        removed += count;
    }
    Ok(removed)
}

fn retained_images(
    store: &Store,
    directory: &Path,
    cached: &BTreeSet<std::path::PathBuf>,
    cancel: &AtomicBool,
) -> Result<BTreeSet<std::path::PathBuf>> {
    let mut retained = BTreeSet::new();
    store.visit_image_urls(cancel, |address| {
        let path = crate::media::cache::path(directory, address);
        if cached.contains(&path) {
            retained.insert(path);
        }
        ensure!(
            retained.len() <= 100_000,
            "Image reference limit reached; no images were removed"
        );
        Ok(())
    })?;
    Ok(retained)
}

fn remove_unchanged(
    path: &Path,
    bytes: u64,
    modified: SystemTime,
) -> Result<bool> {
    let current = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(false);
        }
        Err(error) => return Err(error.into()),
    };
    if !current.is_file()
        || current.len() != bytes
        || current.modified()? != modified
    {
        return Ok(false);
    }
    std::fs::remove_file(path)?;
    Ok(true)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleanup_skips_a_candidate_replaced_after_its_inventory() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let file = directory.path().join("candidate.img");
        std::fs::write(&file, b"old")?;
        let original = file.metadata()?;
        std::fs::write(&file, b"new download")?;
        assert!(!remove_unchanged(
            &file,
            original.len(),
            original.modified()?
        )?);
        assert_eq!(std::fs::read(&file)?, b"new download");
        let current = file.metadata()?;
        assert!(remove_unchanged(&file, current.len(), current.modified()?)?);
        assert!(!remove_unchanged(
            &file,
            current.len(),
            current.modified()?
        )?);
        Ok(())
    }
}

#[cfg(test)]
mod inspection_tests {
    use super::*;

    #[test]
    fn cancelling_an_image_inventory_returns_no_partial_report_and_preserves_files()
    -> Result<()> {
        let directory = tempfile::tempdir()?;
        let store = Store::open(&directory.path().join("library.sqlite3"))?;
        let images = directory.path().join("images");
        for index in 0..20 {
            crate::media::cache::write(
                &images,
                &format!("https://images.example/{index}"),
                b"pixels",
            )?;
        }
        let visited = std::cell::Cell::new(0);
        let check = || {
            visited.set(visited.get() + 1);
            ensure!(visited.get() < 10, "Inventory cancelled");
            Ok(())
        };
        assert!(inspect_checked(&store, &images, Unit::Bytes, &check).is_err());
        assert_eq!(files(&images)?.len(), 20);
        assert!(inspect(&store, &images)?.contains("STORAGE"));
        Ok(())
    }
}
