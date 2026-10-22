//! Pinned original image bytes share the reader decoder and URL validation.

use std::{
    collections::BTreeSet,
    io::Write,
    path::Path,
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

pub(super) fn prepare(
    root: &Path,
    client: &ureq::Agent,
    body: &str,
    available: u64,
    cancel: &AtomicBool,
    progress: &mut Progress,
) -> Result<()> {
    let addresses = addresses(body)?;
    let directory = root.join("offline");
    std::fs::create_dir_all(&directory)?;
    let mut usage = usage(&directory)?;
    let mut decoder = crate::media::Decoder::default();
    for address in addresses {
        ensure!(!cancel.load(Ordering::Relaxed), "Preparation cancelled");
        let bytes = load(root, client, &address, &mut decoder)?;
        ensure!(!cancel.load(Ordering::Relaxed), "Preparation cancelled");
        ensure!(
            progress.bytes + bytes.len() as u64 <= available,
            "Offline package image byte limit reached"
        );
        pin(&directory, &address, &bytes, &mut usage)?;
        progress.bytes += bytes.len() as u64;
        progress.count += 1;
    }
    Ok(())
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

fn pin(
    directory: &Path,
    address: &str,
    bytes: &[u8],
    usage: &mut (u64, usize),
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
            metadata.len()
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            usage.1 += 1;
            0
        }
        Err(error) => return Err(error.into()),
    };
    usage.0 = usage
        .0
        .saturating_sub(previous)
        .saturating_add(bytes.len() as u64);
    ensure!(
        usage.0 <= 536_870_912 && usage.1 <= 4096,
        "Pinned cache limit reached; release the old package first"
    );
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    temporary.persist(path)?;
    Ok(())
}

fn usage(directory: &Path) -> Result<(u64, usize)> {
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
    Ok((bytes, count))
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
