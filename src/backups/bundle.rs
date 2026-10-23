//! Same-filesystem staging avoids a second large copy during publication.
use super::{
    check_cancel,
    manifest::{Entry, Manifest, checksum},
};
use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

pub(super) struct Lock(pub(super) File);
impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

pub(super) fn stage(
    source: &Path,
    parent: &Path,
    now: i64,
    include_offline: bool,
    cancel: &AtomicBool,
    snapshot: impl FnOnce(&Path) -> Result<()>,
) -> Result<(tempfile::TempDir, Manifest)> {
    check_cancel(cancel)?;
    let images = source.parent().context("Library directory")?.join("images");
    let _offline_lock = if include_offline {
        Some(Lock(crate::offline::lock(&images)?))
    } else {
        None
    };
    let staging = tempfile::Builder::new()
        .prefix(".lume-backup-pending-")
        .tempdir_in(parent)?;
    let database = staging.path().join("library.sqlite3");
    snapshot(&database)?;
    let (bytes, sha256) = checksum(&database, cancel)?;
    let mut manifest = Manifest {
        format: 1,
        owner: super::owner(source)?,
        created_at: now,
        include_offline,
        files: vec![Entry {
            name: "library.sqlite3".into(),
            bytes,
            sha256,
        }],
    };
    if include_offline {
        copy_offline(
            &images.join("offline"),
            staging.path(),
            &mut manifest,
            cancel,
        )?;
    }
    manifest.publish(staging.path())?;
    Ok((staging, manifest))
}

fn copy_offline(
    source: &Path,
    destination: &Path,
    manifest: &mut Manifest,
    cancel: &AtomicBool,
) -> Result<()> {
    if !source.try_exists()? {
        return Ok(());
    }
    ensure!(
        source.symlink_metadata()?.is_dir(),
        "Offline cache must be a regular directory"
    );
    let mut total = 0_u64;
    for entry in fs::read_dir(source)? {
        check_cancel(cancel)?;
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !crate::media::cache::owned_filename(&name) {
            continue;
        }
        ensure!(
            entry.file_type()?.is_file(),
            "Offline backup entry must be a regular file"
        );
        let bytes = entry.metadata()?.len();
        total = total
            .checked_add(bytes)
            .context("Offline byte count overflow")?;
        ensure!(
            bytes <= 8_388_608
                && total <= 536_870_912
                && manifest.files.len() <= 4096,
            "Offline backup exceeds cache limits"
        );
        let name = format!("images/offline/{name}");
        let sha256 =
            copy_file(&entry.path(), &destination.join(&name), bytes, cancel)?;
        manifest.files.push(Entry {
            name,
            bytes,
            sha256,
        });
    }
    Ok(())
}

pub(super) fn copy_file(
    source: &Path,
    destination: &Path,
    bytes: u64,
    cancel: &AtomicBool,
) -> Result<String> {
    let parent = destination.parent().context("Backup file directory")?;
    crate::config::publication::prepare_directory(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let mut input = File::open(source)?.take(bytes.saturating_add(1));
    let mut copied = 0_u64;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancel(cancel)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        temporary.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
        copied += count as u64;
    }
    ensure!(copied == bytes, "Backup source size changed during copy");
    temporary.as_file().sync_all()?;
    temporary.persist_noclobber(destination)?;
    Ok(super::manifest::hex(&digest.finalize()))
}

pub(super) fn publish(
    staging: &Path,
    output: &Path,
    names: &[String],
    cancel: &AtomicBool,
) -> Result<()> {
    check_cancel(cancel)?;
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder
        .create(output)
        .context("Backup/restore destination must be a new directory")?;
    for name in names {
        let path = output.join(name);
        crate::config::publication::prepare_directory(
            path.parent().context("Output directory")?,
        )?;
        fs::hard_link(staging.join(name), &path)?;
        File::open(&path)?.sync_all()?;
        File::open(path.parent().context("Output directory")?)?.sync_all()?;
    }
    File::open(output)?.sync_all()?;
    File::open(output.parent().context("Output parent")?)?.sync_all()?;
    Ok(())
}

pub(super) fn copy_checked(
    root: &Path,
    destination: &Path,
    entry: &Entry,
    cancel: &AtomicBool,
) -> Result<()> {
    let hash =
        copy_file(&root.join(&entry.name), destination, entry.bytes, cancel)?;
    ensure!(
        hash == entry.sha256,
        "Backup checksum failed: {}",
        entry.name
    );
    Ok(())
}
