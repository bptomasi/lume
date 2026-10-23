//! Opt-in retention runs on maintenance lanes, never input or rendering paths.
//!
//! Preview acknowledges a policy, not a stale deletion list. Each write batch
//! rechecks personal protections and stops if another connection committed.
//! Compact tombstones prevent publisher refreshes from resurrecting removals.

mod images;

use crate::{config::retention::Retention, store::Store};
use anyhow::Result;
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// Counts and logical bytes; reclaimed SQLite pages are reused, not vacuumed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Articles eligible for removal, or actually removed during application.
    pub articles: usize,
    /// Unprotected Full caches eligible, or actually cleared.
    pub full_texts: usize,
    /// Full body bytes released or eligible.
    pub full_bytes: u64,
    /// Feed body bytes released or eligible.
    pub feed_bytes: u64,
    /// Ordinary image files removed or eligible; pinned copies are untouched.
    pub images: usize,
    /// Ordinary image bytes removed or eligible.
    pub image_bytes: u64,
    /// Cancellation stopped the pass between batches.
    pub cancelled: bool,
    /// A concurrent library change interrupted the captured plan.
    pub interrupted: bool,
}

impl Report {
    pub(crate) fn add(&mut self, other: &Self) {
        self.articles += other.articles;
        self.full_texts += other.full_texts;
        self.full_bytes += other.full_bytes;
        self.feed_bytes += other.feed_bytes;
        self.images += other.images;
        self.image_bytes += other.image_bytes;
    }

    /// Display what is counted without promising a smaller SQLite file.
    pub fn text(&self, preview: bool) -> String {
        format!(
            "{}\nArticles: {}\nFull caches: {} ({} bytes)\nFeed text: {} bytes\nImages: {} ({} bytes)\nCancelled: {}\nInterrupted by library changes: {}\n\nPersonal protections always apply. SQLite reuses freed pages.\n",
            if preview {
                "Retention preview; totals cover all eligible content; apply obeys per-run limits"
            } else {
                "Retention completed"
            },
            self.articles,
            self.full_texts,
            self.full_bytes,
            self.feed_bytes,
            self.images,
            self.image_bytes,
            self.cancelled,
            self.interrupted
        )
    }
}

/// Explain policy activation and scheduling without scanning article bodies.
///
/// # Errors
/// Returns invalid policy or storage errors.
pub fn status(store: &Store) -> Result<String> {
    let policy = store.retention_policy()?;
    Ok(format!(
        "Automatic: {}\nCurrent policy previewed: {}\nInterval: {} hours\nArticle actions per run: {}\nImage files per run: {}\nTransaction limit: {}\nProtect smart-list matches: {}\nLast automatic pass: {}\n\nUse retention preview before retention apply or enabling automatic.\n",
        policy.automatic,
        store.retention_approved(&policy)?,
        policy.interval_hours,
        policy.max_articles_per_run,
        policy.max_images_per_run,
        policy.batch_size,
        policy.protect_smart_lists,
        store
            .retention_state("retention_last_run")?
            .unwrap_or_else(|| "never".into())
    ))
}

/// Preview and remember the exact policy; no article or cache is modified.
///
/// # Errors
/// Returns query, image reference or filesystem errors before acknowledgement.
pub fn preview(store: &Store, directory: &Path, now: i64) -> Result<Report> {
    let plan = store.prepare_retention(store.retention_policy()?, now, None)?;
    let mut report = store.retention_counts()?;
    let candidates = images::candidates(store, &plan, directory, now)?;
    report.images = candidates.len();
    report.image_bytes = candidates.iter().map(|file| file.bytes).sum();
    if !store.retention_current(&plan)? {
        report.interrupted = true;
        return Ok(report);
    }
    store.record_retention_state("retention_preview", &plan.fingerprint)?;
    Ok(report)
}

/// Apply an acknowledged policy in bounded batches; completed work survives errors.
///
/// # Errors
/// Rejects unpreviewed policies or returns SQLite/filesystem errors.
pub fn apply(
    store: &Store,
    directory: &Path,
    now: i64,
    cancel: &AtomicBool,
    active_article: Option<i64>,
) -> Result<Report> {
    if cancel.load(Ordering::Relaxed) {
        return Ok(Report {
            cancelled: true,
            ..Default::default()
        });
    }
    let policy = store.retention_policy()?;
    store.require_retention_preview(&policy)?;
    let plan = store.prepare_retention(policy, now, active_article)?;
    let files = images::candidates(store, &plan, directory, now)?;
    let mut report = apply_articles(store, &plan, now, cancel)?;
    if !report.cancelled && !report.interrupted {
        images::remove(store, &plan, &files, cancel, &mut report)?;
    }
    Ok(report)
}

fn apply_articles(
    store: &Store,
    plan: &crate::store::RetentionPlan,
    now: i64,
    cancel: &AtomicBool,
) -> Result<Report> {
    let mut report = Report::default();
    for action in [
        crate::store::RetentionStep::DeleteArticles,
        crate::store::RetentionStep::ClearFull,
    ] {
        let mut after = 0;
        loop {
            report.cancelled = cancel.load(Ordering::Relaxed);
            let remaining = plan.policy.max_articles_per_run as usize
                - report.articles
                - report.full_texts;
            if report.cancelled || report.interrupted || remaining == 0 {
                break;
            }
            let limit = remaining.min(usize::from(plan.policy.batch_size));
            let Some((batch, next)) =
                store.retention_batch(plan, action, after, limit, now)?
            else {
                report.interrupted = true;
                break;
            };
            after = next;
            report.add(&batch);
            if batch.articles + batch.full_texts < limit {
                break;
            }
            std::thread::yield_now();
        }
    }
    Ok(report)
}

/// Run only when automatic is enabled, previewed and due across process restarts.
///
/// # Errors
/// Returns policy, storage or filesystem errors; an unarmed policy does no work.
pub fn automatic(
    store: &Store,
    directory: &Path,
    now: i64,
    cancel: &AtomicBool,
    active_article: Option<i64>,
) -> Result<Option<Report>> {
    let policy: Retention = store.retention_policy()?;
    if !policy.automatic
        || !policy.has_limits()
        || !store.retention_approved(&policy)?
    {
        return Ok(None);
    }
    let last = store
        .retention_state("retention_last_run")?
        .map(|value| value.parse::<i64>())
        .transpose()?
        .unwrap_or(0);
    if last > 0
        && now < last.saturating_add(i64::from(policy.interval_hours) * 3600)
    {
        return Ok(None);
    }
    let report = apply(store, directory, now, cancel, active_article)?;
    if !report.interrupted {
        store.record_retention_state("retention_last_run", &now.to_string())?;
    }
    Ok(Some(report))
}

/// Schedule the next check from persisted state rather than polling a due date.
/// Unpreviewed policies use a short check so external acknowledgement is observed.
///
/// # Errors
/// Returns policy or persisted scheduling errors.
pub fn next_check(store: &Store, now: i64) -> Result<std::time::Duration> {
    let policy = store.retention_policy()?;
    let interval = i64::from(policy.interval_hours) * 3600;
    if !policy.has_limits() {
        return Ok(std::time::Duration::from_secs(interval as u64));
    }
    if !store.retention_approved(&policy)? {
        return Ok(std::time::Duration::from_secs(60));
    }
    let last = store
        .retention_state("retention_last_run")?
        .map(|value| value.parse::<i64>())
        .transpose()?
        .unwrap_or(0);
    let remaining = last
        .saturating_add(interval)
        .saturating_sub(now)
        .clamp(60, interval);
    Ok(std::time::Duration::from_secs(remaining as u64))
}
