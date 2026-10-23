//! Shared ordinary images survive while any protected or unexpired source needs them.

use crate::{
    retention::Report,
    store::{RetentionPlan, Store},
};
use anyhow::{Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) struct File {
    pub(super) path: PathBuf,
    pub(super) bytes: u64,
    modified: SystemTime,
}

pub(super) fn candidates(
    store: &Store,
    plan: &RetentionPlan,
    directory: &Path,
    now: i64,
    cancel: &AtomicBool,
) -> Result<Vec<File>> {
    super::check_cancel(cancel)?;
    let Some(cutoff) = earliest_expiration(plan, now) else {
        return Ok(Vec::new());
    };
    let mut files = crate::storage::files(directory)?;
    // Even the shortest configured lifetime cannot expire a newer file.
    // Do not hydrate or parse historical bodies when the whole cache is fresh.
    files.retain(|(_, _, modified)| old_enough(*modified, cutoff));
    if files.is_empty() {
        return Ok(Vec::new());
    }
    let cached = files.iter().map(|(path, _, _)| path.clone()).collect();
    let references = references(store, directory, &cached, cancel)?;
    let fallback = plan
        .policy
        .defaults
        .image_days
        .0
        .map_or(0, |days| now.saturating_sub(i64::from(days) * 86_400));
    Ok(expired_files(files, &references, fallback))
}

fn earliest_expiration(plan: &RetentionPlan, now: i64) -> Option<i64> {
    let default = plan.policy.defaults.image_days;
    std::iter::once(default)
        .chain(
            plan.policy
                .sources
                .iter()
                .map(|source| source.image_days.unwrap_or(default)),
        )
        .filter_map(|age| age.0)
        .min()
        .map(|days| now.saturating_sub(i64::from(days) * 86_400).max(0))
}

fn old_enough(modified: SystemTime, cutoff: i64) -> bool {
    cutoff > 0
        && modified
            .duration_since(UNIX_EPOCH)
            .is_ok_and(|age| age.as_secs() < cutoff as u64)
}

fn expired_files(
    files: Vec<(PathBuf, u64, SystemTime)>,
    references: &BTreeMap<PathBuf, i64>,
    fallback: i64,
) -> Vec<File> {
    let started = SystemTime::now();
    let mut candidates = Vec::new();
    for (path, bytes, modified) in files {
        let cutoff = references.get(&path).copied().unwrap_or(fallback);
        if modified < started && old_enough(modified, cutoff) {
            candidates.push(File {
                path,
                bytes,
                modified,
            });
        }
    }
    candidates.sort_by(|left, right| {
        left.modified
            .cmp(&right.modified)
            .then_with(|| left.path.cmp(&right.path))
    });
    candidates
}

fn references(
    store: &Store,
    directory: &Path,
    cached: &BTreeSet<PathBuf>,
    cancel: &AtomicBool,
) -> Result<BTreeMap<PathBuf, i64>> {
    let mut references = BTreeMap::new();
    if !store.refresh_image_references(cancel)? {
        return Err(super::Canceled.into());
    }
    store.visit_retention_images(|address, cutoff| {
        super::check_cancel(cancel)?;
        let path = crate::media::cache::path(directory, address);
        if cached.contains(&path) {
            references
                .entry(path)
                .and_modify(|existing: &mut i64| {
                    *existing = (*existing).min(cutoff)
                })
                .or_insert(cutoff);
        }
        ensure!(
            references.len() <= 100_000,
            "Image reference limit reached; retention removed no files"
        );
        Ok(())
    })?;
    Ok(references)
}

pub(super) fn remove(
    store: &Store,
    plan: &RetentionPlan,
    candidates: &[File],
    cancel: &AtomicBool,
    report: &mut Report,
) -> Result<()> {
    let maximum = candidates
        .len()
        .min(plan.policy.max_images_per_run as usize);
    for batch in
        candidates[..maximum].chunks(usize::from(plan.policy.batch_size))
    {
        report.cancelled = cancel.load(Ordering::Relaxed);
        if report.cancelled {
            break;
        }
        let current = store.retention_image_batch(plan, || {
            for file in batch {
                let metadata = match file.path.symlink_metadata() {
                    Ok(metadata) => metadata,
                    Err(error)
                        if error.kind() == std::io::ErrorKind::NotFound =>
                    {
                        continue;
                    }
                    Err(error) => return Err(error.into()),
                };
                if metadata.is_file()
                    && metadata.len() == file.bytes
                    && metadata.modified()? == file.modified
                {
                    std::fs::remove_file(&file.path)?;
                    report.images += 1;
                    report.image_bytes += file.bytes;
                }
            }
            Ok(())
        })?;
        if !current {
            report.interrupted = true;
            break;
        }
        std::thread::yield_now();
    }
    Ok(())
}
