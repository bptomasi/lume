//! One bounded image lane; drop disconnects queues without waiting for HTTP.

use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::Duration,
};

use anyhow::{Context, Result, ensure};

use super::{Decoder, ImageKey, MAX_BYTES, Thumbnail, cache};

pub(crate) struct Completion {
    pub(crate) key: ImageKey,
    pub(crate) result: Result<Thumbnail, String>,
}

pub(crate) struct Worker {
    requests: SyncSender<ImageKey>,
    results: Receiver<Completion>,
}

impl Worker {
    pub(crate) fn start(directory: PathBuf, offline: bool) -> Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<ImageKey>(1);
        let (sender, results) = mpsc::sync_channel(1);
        thread::Builder::new()
            .name("lume-images".into())
            .spawn(move || {
                let mut client = None;
                let mut decoder = Decoder::default();
                while let Ok(key) = receiver.recv() {
                    let result = load(
                        &directory,
                        offline,
                        &mut client,
                        &mut decoder,
                        &key,
                    )
                    .map_err(|error| format!("{error:#}"));
                    if sender.send(Completion { key, result }).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self { requests, results })
    }

    pub(crate) fn submit(&self, key: ImageKey) -> Result<()> {
        self.requests
            .try_send(key)
            .context("Image worker is busy or stopped")
    }

    pub(crate) fn poll(&self) -> Result<Option<Completion>> {
        match self.results.try_recv() {
            Ok(completion) => Ok(Some(completion)),
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                anyhow::bail!("Image worker stopped; text remains available")
            }
        }
    }
}

fn load(
    directory: &std::path::Path,
    offline: bool,
    client: &mut Option<ureq::Agent>,
    decoder: &mut Decoder,
    key: &ImageKey,
) -> Result<Thumbnail> {
    crate::source::feed_url(&key.url)?;
    let cached = cache::read(directory, &key.url).and_then(|bytes| {
        bytes.map(|bytes| decoder.render(&bytes, key)).transpose()
    });
    match cached {
        Ok(Some(image)) => return Ok(image),
        Err(error) if offline => {
            return Err(error.context("Offline image cache"));
        }
        _ => {}
    }
    ensure!(!offline, "Image unavailable in offline cache");
    let client = client.get_or_insert_with(crate::source::agent);
    let bytes = download(client, &key.url)?;
    let mut image = decoder.render(&bytes, key)?;
    image.cache_warning = cache::write(directory, &key.url, &bytes)
        .err()
        .map(|error| error.to_string());
    Ok(image)
}

pub(crate) fn download(client: &ureq::Agent, address: &str) -> Result<Vec<u8>> {
    let mut response = client
        .get(address)
        .header(
            "Accept",
            "image/png, image/jpeg, image/webp, image/gif, image/svg+xml",
        )
        .config()
        .timeout_global(Some(Duration::from_secs(6)))
        .build()
        .call()
        .context("Could not download image")?;
    ensure!(
        response.status() == 200,
        "Image returned HTTP {}",
        response.status()
    );
    crate::http_body::read(response.body_mut(), MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn disconnected_worker_is_reported_instead_of_remaining_pending_forever() {
        let (requests, _receiver) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(1);
        let worker = Worker { requests, results };
        assert!(matches!(worker.poll(), Ok(None)));

        drop(sender);

        assert!(worker.poll().is_err());
    }
}
