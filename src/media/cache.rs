//! Disposable, atomically published image files; publisher state stays in SQLite.

use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

use super::MAX_BYTES;

const DISK_BUDGET: u64 = 64 * 1024 * 1024;
const FILE_BUDGET: usize = 128;

/// Stable cache path for a validated URL; no publisher string becomes a path.
pub fn path(directory: &Path, address: &str) -> PathBuf {
    let digest = Sha256::digest(address.as_bytes());
    let name = digest
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    directory.join(format!("{name}.img"))
}

/// Read bounded cached bytes; a missing entry is an ordinary cache miss.
///
/// # Errors
/// Returns permission, I/O or oversized-cache errors without network access.
pub fn read(directory: &Path, address: &str) -> Result<Option<Vec<u8>>> {
    let file = match fs::File::open(path(directory, address)) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    ensure!(bytes.len() <= MAX_BYTES, "Cached image exceeds 8 MiB");
    Ok(Some(bytes))
}

/// Atomically publish validated image bytes and evict oldest owned cache files.
///
/// # Errors
/// Returns oversized input, permission or publication/eviction failures.
pub fn write(directory: &Path, address: &str, bytes: &[u8]) -> Result<()> {
    ensure!(bytes.len() <= MAX_BYTES, "Image exceeds 8 MiB");
    fs::create_dir_all(directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    temporary.persist(path(directory, address))?;
    prune(directory)
}

fn prune(directory: &Path) -> Result<()> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if !owned_filename(&name) || !entry.file_type()?.is_file() {
            continue;
        }
        let metadata = entry.metadata()?;
        files.push((metadata.modified()?, entry.path(), metadata.len()));
    }
    files.sort();
    let mut bytes = files.iter().map(|(_, _, size)| size).sum::<u64>();
    let mut count = files.len();
    for (_, path, size) in files {
        if bytes <= DISK_BUDGET && count <= FILE_BUDGET {
            break;
        }
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        bytes = bytes.saturating_sub(size);
        count -= 1;
    }
    Ok(())
}

fn owned_filename(name: &str) -> bool {
    name.strip_suffix(".img").is_some_and(|stem| {
        stem.len() == 64 && stem.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}
