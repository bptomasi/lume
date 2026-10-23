//! Opt-in local bundles: coherent WAL snapshots, checked offline copies and owned rotation.
//! No worker runs while Lume is closed. Ordinary SQLite snapshots remain compatible.
mod bundle;
mod manifest;
mod rotation;

use crate::store::Store;
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

#[derive(Debug)]
struct Canceled;
impl std::fmt::Display for Canceled {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("Backup cancelled before publication")
    }
}
impl std::error::Error for Canceled {}
pub(crate) fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(Canceled.into());
    }
    Ok(())
}
fn owner(source: &Path) -> Result<String> {
    Ok(manifest::hex(&Sha256::digest(
        source.canonicalize()?.as_os_str().as_encoded_bytes(),
    )))
}

/// Describe opt-in preferences and the durable successful-publication timestamp.
///
/// # Errors
/// Returns invalid configuration or SQLite failures.
pub fn status(store: &Store) -> Result<String> {
    let policy = store.backup_policy()?;
    let source = store.database_path()?;
    Ok(format!(
        "Automatic backups: {}\nInterval: {} hours\nRetain: {} complete bundles\nInclude offline images: {}\nDirectory: {}\nLast successful publication: {}\n\nConfigure [backups] in TOML. Automatic work runs while Lume is active.\n:backup now creates a managed bundle using the same rotation policy.\nRestore a bundle with lume restore BUNDLE NEW_DIRECTORY.\nSnapshots contain private articles, notes and API tokens.\n",
        policy.automatic,
        policy.interval_hours,
        policy.keep,
        policy.include_offline,
        directory(&policy, &source)?.display(),
        store
            .backup_last_run()?
            .map_or_else(|| "never".into(), |time| time.to_string())
    ))
}

fn directory(
    policy: &crate::config::backups::Backups,
    source: &Path,
) -> Result<PathBuf> {
    if policy.directory.as_os_str().is_empty() {
        Ok(source
            .parent()
            .context("Library directory")?
            .join("backups"))
    } else {
        Ok(policy.directory.clone())
    }
}

/// Time until the next automatic attempt; cancellation/failure retries after a minute.
///
/// # Errors
/// Returns invalid policy or SQLite failures.
pub fn next_check(store: &Store, now: i64) -> Result<Duration> {
    let policy = store.backup_policy()?;
    let deadline = store
        .backup_last_run()?
        .unwrap_or(0)
        .saturating_add(i64::from(policy.interval_hours) * 3600);
    Ok(Duration::from_secs(
        deadline.saturating_sub(now).max(60) as u64
    ))
}

/// Run a due automatic backup, preserving the interval across process restarts.
///
/// # Errors
/// Returns lock, snapshot, filesystem or rotation failures; existing bundles survive.
pub fn automatic(
    store: &Store,
    now: i64,
    cancel: &AtomicBool,
) -> Result<Option<PathBuf>> {
    let policy = store.backup_policy()?;
    if !policy.automatic || !due(store, &policy, now)? {
        return Ok(None);
    }
    attempt(store, now, cancel, true)
}

/// Create a managed bundle even when automatic work is disabled, then rotate owned bundles.
///
/// # Errors
/// Returns snapshot/rotation failures. Cancellation before publication returns None.
pub fn run(
    store: &Store,
    now: i64,
    cancel: &AtomicBool,
) -> Result<Option<PathBuf>> {
    attempt(store, now, cancel, false)
}

fn attempt(
    store: &Store,
    now: i64,
    cancel: &AtomicBool,
    automatic: bool,
) -> Result<Option<PathBuf>> {
    match managed(store, now, cancel, automatic) {
        Err(error) if error.is::<Canceled>() => Ok(None),
        result => result,
    }
}
fn due(
    store: &Store,
    policy: &crate::config::backups::Backups,
    now: i64,
) -> Result<bool> {
    Ok(store.backup_last_run()?.is_none_or(|last| {
        now >= last.saturating_add(i64::from(policy.interval_hours) * 3600)
    }))
}

fn managed(
    store: &Store,
    now: i64,
    cancel: &AtomicBool,
    automatic: bool,
) -> Result<Option<PathBuf>> {
    check_cancel(cancel)?;
    let policy = store.backup_policy()?;
    let source = store.database_path()?;
    let root = directory(&policy, &source)?;
    crate::config::publication::prepare_directory(&root)?;
    anyhow::ensure!(
        root.symlink_metadata()?.is_dir(),
        "Backup directory must not be a symlink"
    );
    let owner = owner(&source)?;
    let _lock = rotation::lock(&root, &owner)?;
    if automatic && !due(store, &policy, now)? {
        return Ok(None);
    }
    let (staging, manifest) = bundle::stage(
        &source,
        &root,
        now,
        policy.include_offline,
        cancel,
        |path| store.backup_to_cancellable(path, cancel),
    )?;
    let suffix = staging
        .path()
        .file_name()
        .context("Backup staging directory")?
        .to_string_lossy();
    let output = root.join(format!(
        "lume-backup-{}-{now:020}-{}",
        &owner[..16],
        suffix.trim_start_matches(".lume-backup-pending-")
    ));
    publish_bundle(staging.path(), &output, &manifest, cancel)?;
    store.record_backup_run(now)?;
    rotation::rotate(&root, &manifest.owner, &output, policy.keep, cancel)?;
    Ok(Some(output))
}

fn publish_bundle(
    staging: &Path,
    output: &Path,
    manifest: &manifest::Manifest,
    cancel: &AtomicBool,
) -> Result<()> {
    let names = manifest
        .files
        .iter()
        .map(|file| file.name.clone())
        .chain(std::iter::once("manifest.json".into()))
        .collect::<Vec<_>>();
    bundle::publish(staging, output, &names, cancel)
}

/// Make an explicit no-clobber directory bundle from a read-only existing library.
///
/// # Errors
/// Returns invalid source, lock, checksum or filesystem failures.
pub fn snapshot_existing(
    source: &Path,
    output: &Path,
    include_offline: bool,
) -> Result<()> {
    anyhow::ensure!(!output.try_exists()?, "Backup destination already exists");
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let cancel = AtomicBool::new(false);
    let (staging, manifest) = bundle::stage(
        source,
        parent,
        chrono::Utc::now().timestamp(),
        include_offline,
        &cancel,
        |path| Store::snapshot_existing(source, path),
    )?;
    publish_bundle(staging.path(), output, &manifest, &cancel)
}

/// Verify every checksum before restoring a bundle into a new isolated directory.
///
/// # Errors
/// Returns incomplete/modified bundles or existing destination failures.
pub fn restore(source: &Path, output: &Path) -> Result<()> {
    anyhow::ensure!(
        !output.try_exists()?,
        "Restore destination already exists"
    );
    let cancel = AtomicBool::new(false);
    let manifest = manifest::Manifest::read(source)?;
    let parent = output
        .parent()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let staging = tempfile::tempdir_in(parent)?;
    let restored = staging.path().join("restored");
    let database = manifest
        .files
        .iter()
        .find(|file| file.name == "library.sqlite3")
        .context("Backup database entry")?;
    let staged_database = staging.path().join("snapshot.sqlite3");
    bundle::copy_checked(source, &staged_database, database, &cancel)?;
    Store::restore_database_snapshot(
        &staged_database,
        &restored,
        manifest.include_offline,
    )?;
    let mut names = vec!["library.sqlite3".into(), "config.toml".into()];
    for file in manifest
        .files
        .iter()
        .filter(|file| file.name != "library.sqlite3")
    {
        bundle::copy_checked(
            source,
            &restored.join(&file.name),
            file,
            &cancel,
        )?;
        names.push(file.name.clone());
    }
    bundle::publish(&restored, output, &names, &cancel)
}

/// Verify a complete bundle without modifying or restoring it.
///
/// # Errors
/// Returns missing/extra files, checksum mismatches or filesystem failures.
pub fn verify(path: &Path) -> Result<()> {
    let manifest = manifest::Manifest::read(path)?;
    manifest.verify(path, &AtomicBool::new(false))
}
