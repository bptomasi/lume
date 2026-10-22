//! Pinned original image bytes share the reader decoder and URL validation.

use std::{
    collections::BTreeSet,
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Result, ensure};
use pulldown_cmark::{Event, Parser, Tag};

/// Discover bounded absolute publisher image URLs using the qualified parser.
pub(crate) fn addresses(body: &str) -> Result<BTreeSet<String>> {
    let mut addresses = BTreeSet::new();
    for event in Parser::new(body) {
        if let Event::Start(Tag::Image { dest_url, .. }) = event {
            addresses.insert(crate::source::feed_url(&dest_url)?.to_string());
            ensure!(
                addresses.len() <= 64,
                "Article contains more than 64 images"
            );
        }
    }
    Ok(addresses)
}

#[derive(Default)]
pub(super) struct Progress {
    pub(super) bytes: u64,
    pub(super) count: usize,
}

/// Package-local accounting is valid while the caller holds the offline lock.
/// Scan once and share the decoder; publication updates usage only on success.
pub(super) struct Preparation<'a> {
    root: &'a Path,
    client: &'a ureq::Agent,
    directory: PathBuf,
    usage: Usage,
    decoder: crate::media::Decoder,
}

impl<'a> Preparation<'a> {
    pub(super) fn open(
        root: &'a Path,
        client: &'a ureq::Agent,
    ) -> Result<Self> {
        let directory = root.join("offline");
        std::fs::create_dir_all(&directory)?;
        Ok(Self {
            usage: usage(&directory)?,
            root,
            client,
            directory,
            decoder: crate::media::Decoder::default(),
        })
    }

    pub(super) fn prepare(
        &mut self,
        body: &str,
        available: u64,
        cancel: &AtomicBool,
        progress: &mut Progress,
    ) -> Result<()> {
        for address in addresses(body)? {
            ensure!(!cancel.load(Ordering::Relaxed), "Preparation cancelled");
            let bytes =
                load(self.root, self.client, &address, &mut self.decoder)?;
            ensure!(!cancel.load(Ordering::Relaxed), "Preparation cancelled");
            ensure!(
                progress.bytes + bytes.len() as u64 <= available,
                "Offline package image byte limit reached"
            );
            pin(&self.directory, &address, &bytes, &mut self.usage)?;
            progress.bytes += bytes.len() as u64;
            progress.count += 1;
        }
        Ok(())
    }
}

fn load(
    root: &Path,
    client: &ureq::Agent,
    address: &str,
    decoder: &mut crate::media::Decoder,
) -> Result<Vec<u8>> {
    let key = crate::media::ImageKey {
        url: address.into(),
        width: 1,
        height: 1,
        background: [22, 22, 24],
        detail: None,
    };
    if let Ok(Some(bytes)) = crate::media::cache::read(root, address)
        && decoder.render(&bytes, &key).is_ok()
    {
        return Ok(bytes);
    }
    let bytes = crate::media::download(client, address)?;
    decoder.render(&bytes, &key)?;
    Ok(bytes)
}

#[derive(Default)]
struct Usage {
    bytes: u64,
    count: usize,
}

fn pin(
    directory: &Path,
    address: &str,
    bytes: &[u8],
    usage: &mut Usage,
) -> Result<()> {
    let path = crate::media::cache::path(directory, address);
    let previous = match std::fs::symlink_metadata(&path) {
        Ok(metadata) => {
            ensure!(
                metadata.file_type().is_file(),
                "Pinned cache entry is not a regular file"
            );
            if let Ok(Some(current)) =
                crate::media::cache::read(directory, address)
                && current == bytes
            {
                return Ok(());
            }
            Some(metadata.len())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(error.into()),
    };
    let updated = Usage {
        bytes: usage
            .bytes
            .saturating_sub(previous.unwrap_or(0))
            .saturating_add(bytes.len() as u64),
        count: usage.count + usize::from(previous.is_none()),
    };
    ensure!(
        updated.bytes <= 536_870_912 && updated.count <= 4096,
        "Pinned cache limit reached; release the old package first"
    );
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    temporary.persist(path)?;
    *usage = updated;
    Ok(())
}

fn usage(directory: &Path) -> Result<Usage> {
    let mut bytes = 0_u64;
    let mut count = 0;
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && crate::media::cache::owned_filename(
                &entry.file_name().to_string_lossy(),
            )
        {
            bytes = bytes.saturating_add(entry.metadata()?.len());
            count += 1;
        }
    }
    Ok(Usage { bytes, count })
}

pub(super) fn release(directory: &Path) -> Result<()> {
    if !directory.try_exists()? {
        return Ok(());
    }
    for entry in std::fs::read_dir(directory)? {
        let entry = entry?;
        if entry.file_type()?.is_file()
            && crate::media::cache::owned_filename(
                &entry.file_name().to_string_lossy(),
            )
        {
            std::fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejected_pins_do_not_reserve_capacity_for_later_images() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let history = crate::media::cache::path(
            directory.path(),
            "https://example.org/history.svg",
        );
        // A sparse unreferenced entry exercises the global byte budget without
        // allocating 512 MiB. Requested entries still require decoder checks.
        let file = std::fs::File::create(history)?;
        file.set_len(536_870_912 - 4)?;
        let mut usage = usage(directory.path())?;
        let address = "https://example.org/new.svg";

        assert!(
            pin(directory.path(), address, b"too large", &mut usage).is_err()
        );

        assert_eq!(usage.bytes, 536_870_912 - 4);
        assert_eq!(usage.count, 1);
        assert!(!crate::media::cache::path(directory.path(), address).exists());
        pin(directory.path(), address, b"fits", &mut usage)?;
        assert_eq!(usage.bytes, 536_870_912);
        assert_eq!(usage.count, 2);
        Ok(())
    }
}
