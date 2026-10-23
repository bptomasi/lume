//! A manifest is published last; readers reject incomplete or modified bundles.
use super::check_cancel;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs::File,
    io::{Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Manifest {
    pub(super) format: u8,
    pub(super) owner: String,
    pub(super) created_at: i64,
    pub(super) include_offline: bool,
    pub(super) files: Vec<Entry>,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Entry {
    pub(super) name: String,
    pub(super) bytes: u64,
    pub(super) sha256: String,
}

pub(super) fn checksum(
    path: &Path,
    cancel: &AtomicBool,
) -> Result<(u64, String)> {
    ensure!(
        path.symlink_metadata()?.is_file(),
        "Backup entry is not a regular file"
    );
    let mut input = File::open(path)?;
    let mut digest = Sha256::new();
    let mut bytes = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        check_cancel(cancel)?;
        let count = input.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        bytes += count as u64;
        digest.update(&buffer[..count]);
    }
    Ok((bytes, hex(&digest.finalize())))
}

impl Manifest {
    pub(super) fn read(root: &Path) -> Result<Self> {
        ensure!(
            root.symlink_metadata()?.is_dir(),
            "Backup bundle is not a regular directory"
        );
        let path = root.join("manifest.json");
        ensure!(
            path.symlink_metadata()?.is_file(),
            "Backup manifest is not a regular file"
        );
        let mut bytes = Vec::new();
        File::open(path)?.take(2_097_153).read_to_end(&mut bytes)?;
        ensure!(bytes.len() <= 2_097_152, "Backup manifest exceeds 2 MiB");
        let manifest: Self = serde_json::from_slice(&bytes)?;
        manifest.validate()?;
        manifest.layout(root)?;
        Ok(manifest)
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            self.format == 1 && digest(&self.owner),
            "Unknown backup manifest format/owner"
        );
        ensure!(
            self.files.len() <= 4097,
            "Backup manifest exceeds file limit"
        );
        let mut names = BTreeSet::new();
        let mut offline_bytes = 0_u64;
        for file in &self.files {
            ensure!(names.insert(file.name.as_str()), "Duplicate backup entry");
            ensure!(
                allowed(&file.name) && digest(&file.sha256),
                "Invalid backup entry"
            );
            if file.name != "library.sqlite3" {
                offline_bytes = offline_bytes
                    .checked_add(file.bytes)
                    .context("Offline backup byte overflow")?;
                ensure!(
                    offline_bytes <= 536_870_912,
                    "Offline backup exceeds 512 MiB"
                );
                ensure!(
                    self.include_offline && file.bytes <= 8_388_608,
                    "Invalid offline backup entry"
                );
            }
        }
        ensure!(
            names.contains("library.sqlite3"),
            "Backup is missing its database"
        );
        Ok(())
    }

    pub(super) fn layout(&self, root: &Path) -> Result<()> {
        let expected = self
            .files
            .iter()
            .map(|file| file.name.as_str())
            .chain(std::iter::once("manifest.json"))
            .collect::<BTreeSet<_>>();
        let observed = observed(root, &expected)?;
        ensure!(
            observed.len() == expected.len(),
            "Backup bundle is incomplete"
        );
        for file in &self.files {
            ensure!(
                root.join(&file.name).symlink_metadata()?.len() == file.bytes,
                "Backup size changed: {}",
                file.name
            );
        }
        Ok(())
    }

    pub(super) fn verify(
        &self,
        root: &Path,
        cancel: &AtomicBool,
    ) -> Result<()> {
        for file in &self.files {
            let (bytes, hash) = checksum(&root.join(&file.name), cancel)?;
            ensure!(
                bytes == file.bytes && hash == file.sha256,
                "Backup checksum failed: {}",
                file.name
            );
        }
        Ok(())
    }

    pub(super) fn publish(&self, root: &Path) -> Result<()> {
        let mut temporary = tempfile::NamedTempFile::new_in(root)?;
        temporary.write_all(&serde_json::to_vec_pretty(self)?)?;
        temporary.as_file().sync_all()?;
        temporary.persist_noclobber(root.join("manifest.json"))?;
        File::open(root)?.sync_all()?;
        Ok(())
    }
}
fn allowed(name: &str) -> bool {
    name == "library.sqlite3"
        || name
            .strip_prefix("images/offline/")
            .is_some_and(crate::media::cache::owned_filename)
}
fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

fn observed(
    root: &Path,
    expected: &BTreeSet<&str>,
) -> Result<BTreeSet<String>> {
    let mut observed = BTreeSet::new();
    let mut directories = vec![root.to_path_buf()];
    while let Some(directory) = directories.pop() {
        for entry in std::fs::read_dir(directory)? {
            let entry = entry?;
            let path = entry.path();
            let name = path
                .strip_prefix(root)?
                .to_str()
                .context("Invalid backup filename")?
                .replace('\\', "/");
            if entry.file_type()?.is_dir()
                && matches!(name.as_str(), "images" | "images/offline")
            {
                directories.push(path);
            } else {
                ensure!(
                    entry.file_type()?.is_file()
                        && expected.contains(name.as_str()),
                    "Unexpected backup entry: {name}"
                );
                observed.insert(name);
            }
        }
    }
    Ok(observed)
}
