//! Two bounded downloads share one memory-bounded decoder.
//! Dropping the worker disconnects queues without waiting for HTTP.

use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, ensure};

use super::{Decoder, ImageKey, MAX_BYTES, Thumbnail, cache};

pub(crate) struct Completion {
    pub(crate) key: ImageKey,
    pub(crate) result: Result<Thumbnail, String>,
}

struct Request {
    key: ImageKey,
    // Only application-owned formula slots can submit local bytes.
    generated: Option<Arc<[u8]>>,
}

pub(crate) struct Worker {
    requests: SyncSender<Request>,
    results: Receiver<Completion>,
}

impl Worker {
    pub(crate) fn start(directory: PathBuf, offline: bool) -> Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<Request>(2);
        let (sender, results) = mpsc::sync_channel(2);
        let receiver = Arc::new(Mutex::new(receiver));
        let decoder = Arc::new(Mutex::new(Decoder::default()));
        for lane in 0..2 {
            let (receiver, decoder) = (receiver.clone(), decoder.clone());
            let (directory, sender) = (directory.clone(), sender.clone());
            thread::Builder::new()
                .name(format!("lume-images-{lane}"))
                .spawn(move || {
                    run(directory, offline, receiver, sender, decoder)
                })?;
        }
        Ok(Self { requests, results })
    }

    pub(crate) fn submit(
        &self,
        key: ImageKey,
        generated: Option<Arc<[u8]>>,
    ) -> Result<()> {
        self.requests
            .try_send(Request { key, generated })
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

fn run(
    directory: PathBuf,
    offline: bool,
    receiver: Arc<Mutex<Receiver<Request>>>,
    sender: SyncSender<Completion>,
    decoder: Arc<Mutex<Decoder>>,
) {
    let mut client = None;
    loop {
        let key = match receiver.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => break,
        };
        let Ok(Request { key, generated }) = key else {
            break;
        };
        let result = match generated {
            Some(bytes) => render(&decoder, &bytes, &key),
            None => load(&directory, offline, &mut client, &decoder, &key),
        }
        .map_err(|error| format!("{error:#}"));
        if sender.send(Completion { key, result }).is_err() {
            break;
        }
    }
}

fn render(
    decoder: &Mutex<Decoder>,
    bytes: &[u8],
    key: &ImageKey,
) -> Result<Thumbnail> {
    let mut decoder = decoder
        .lock()
        .map_err(|_| anyhow::anyhow!("Image decoder stopped"))?;
    decoder.render(bytes, key)
}

fn load(
    directory: &std::path::Path,
    offline: bool,
    client: &mut Option<ureq::Agent>,
    decoder: &Mutex<Decoder>,
    key: &ImageKey,
) -> Result<Thumbnail> {
    crate::source::feed_url(&key.url)?;
    let cached = cache::read(directory, &key.url).and_then(|bytes| {
        bytes.map(|bytes| render(decoder, &bytes, key)).transpose()
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
    let mut image = render(decoder, &bytes, key)?;
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
