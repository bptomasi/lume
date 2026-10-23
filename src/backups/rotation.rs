//! Rotation only removes manifest-listed files from this library's complete bundles.
use super::{bundle::Lock, manifest::Manifest};
use anyhow::{Context, Result, ensure};
use std::{
    fs::{self, OpenOptions},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) fn lock(root: &Path, owner: &str) -> Result<Lock> {
    let path = root.join(format!(".lume-backup-{owner}.lock"));
    if path.try_exists()? {
        ensure!(
            path.symlink_metadata()?.is_file(),
            "Backup lock is not a regular file"
        );
    }
    let mut options = OpenOptions::new();
    options.create(true).read(true).write(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options.open(path)?;
    file.try_lock()
        .context("Another backup is running for this library")?;
    Ok(Lock(file))
}

pub(super) fn rotate(
    root: &Path,
    owner: &str,
    newest: &Path,
    keep: u16,
    cancel: &AtomicBool,
) -> Result<()> {
    let prefix = format!("lume-backup-{}-", &owner[..16]);
    let mut candidates = Vec::new();
    for entry in fs::read_dir(root)? {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let entry = entry?;
        if entry.path() == newest
            || !entry.file_type()?.is_dir()
            || !entry.file_name().to_string_lossy().starts_with(&prefix)
        {
            continue;
        }
        if let Ok(manifest) = Manifest::read(&entry.path())
            && manifest.owner == owner
        {
            candidates.push((manifest.created_at, entry.path(), manifest));
        }
    }
    candidates.sort_by(|left, right| {
        right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1))
    });
    for (_, path, manifest) in candidates
        .into_iter()
        .skip(usize::from(keep.saturating_sub(1)))
    {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        remove(&path, &manifest)?;
    }
    Ok(())
}
fn remove(path: &Path, manifest: &Manifest) -> Result<()> {
    manifest.layout(path)?;
    for file in &manifest.files {
        fs::remove_file(path.join(&file.name))?;
    }
    for directory in ["images/offline", "images"] {
        let directory = path.join(directory);
        if directory.try_exists()? {
            fs::remove_dir(directory)?;
        }
    }
    fs::remove_file(path.join("manifest.json"))?;
    fs::remove_dir(path)?;
    fs::File::open(path.parent().context("Backup root")?)?.sync_all()?;
    Ok(())
}
