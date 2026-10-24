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
mod http;
mod request;
use request::Request;

pub(crate) struct Completion {
    pub(crate) key: ImageKey,
    pub(crate) result: Result<Thumbnail, String>,
}

pub(crate) struct Worker {
    requests: SyncSender<Arc<Request>>,
    results: Receiver<Completion>,
    jobs: Vec<Arc<Request>>,
}

impl Worker {
    pub(crate) fn start(directory: PathBuf, offline: bool) -> Result<Self> {
        let (requests, receiver) = mpsc::sync_channel::<Arc<Request>>(2);
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
        Ok(Self {
            requests,
            results,
            jobs: Vec::new(),
        })
    }

    pub(crate) fn submit(
        &mut self,
        key: ImageKey,
        generated: Option<Arc<[u8]>>,
    ) -> Result<bool> {
        for job in &self.jobs {
            if job.url == key.url && job.retarget(Some(key.clone()))? {
                return Ok(true);
            }
        }
        let request = Arc::new(Request::new(key, generated));
        match self.requests.try_send(request.clone()) {
            Ok(()) => {
                self.jobs.push(request);
                Ok(true)
            }
            Err(mpsc::TrySendError::Full(_)) => Ok(false),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                anyhow::bail!("Image worker stopped")
            }
        }
    }

    pub(crate) fn retain(&mut self, wanted: &[ImageKey]) -> Result<()> {
        let mut retained = Vec::with_capacity(self.jobs.len());
        for job in self.jobs.drain(..) {
            let current = job.key()?;
            let key =
                current.filter(|key| wanted.contains(key)).or_else(|| {
                    wanted.iter().find(|key| key.url == job.url).cloned()
                });
            if job.retarget(key)? {
                retained.push(job);
            }
        }
        self.jobs = retained;
        Ok(())
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
    receiver: Arc<Mutex<Receiver<Arc<Request>>>>,
    sender: SyncSender<Completion>,
    decoder: Arc<Mutex<Decoder>>,
) {
    let mut client = None;
    loop {
        let request = match receiver.lock() {
            Ok(receiver) => receiver.recv(),
            Err(_) => break,
        };
        let Ok(request) = request else {
            break;
        };
        let result = load(&directory, offline, &mut client, &decoder, &request);
        let completion = match result {
            Ok(Some((key, image))) => Some(Completion {
                key,
                result: Ok(image),
            }),
            Ok(None) => None,
            Err(error) => request.key().ok().flatten().map(|key| Completion {
                key,
                result: Err(format!("{error:#}")),
            }),
        };
        let wanted = request.finish().unwrap_or(false);
        if wanted
            && let Some(completion) = completion
            && sender.send(completion).is_err()
        {
            break;
        }
    }
}

fn render(
    decoder: &Mutex<Decoder>,
    bytes: &[u8],
    request: &Request,
) -> Result<Option<(ImageKey, Thumbnail)>> {
    render_with(decoder, bytes, request, super::decode::prepare_cancellable)
}

fn render_with(
    decoder: &Mutex<Decoder>,
    bytes: &[u8],
    request: &Request,
    mut prepare: impl FnMut(
        image::DynamicImage,
        (u32, u32),
        &ImageKey,
        Option<&crate::cancellation::Cancellation>,
    ) -> Result<Thumbnail>,
) -> Result<Option<(ImageKey, Thumbnail)>> {
    loop {
        let (key, cancel, image, dimensions) = {
            let mut decoder = decoder
                .lock()
                .map_err(|_| anyhow::anyhow!("Image decoder stopped"))?;
            let Some((key, cancel)) = request.snapshot()? else {
                return Ok(None);
            };
            let (image, dimensions) = decoder.fit(bytes, &key)?;
            (key, cancel, image, dimensions)
        };
        if cancel.is_cancelled() {
            continue;
        }
        let image = prepare(image, dimensions, &key, Some(&cancel));
        // A new crop shares these source bytes and the decoder's source cache.
        // Never publish a partial or obsolete terminal payload.
        if cancel.is_cancelled() {
            continue;
        }
        return Ok(Some((key, image?)));
    }
}

fn load(
    directory: &std::path::Path,
    offline: bool,
    client: &mut Option<http::Client>,
    decoder: &Mutex<Decoder>,
    request: &Request,
) -> Result<Option<(ImageKey, Thumbnail)>> {
    if request.key()?.is_none() {
        return Ok(None);
    }
    if let Some(bytes) = &request.generated {
        return render(decoder, bytes, request);
    }
    crate::source::feed_url(&request.url)?;
    let cached = cache::read(directory, &request.url).and_then(|bytes| {
        bytes
            .map(|bytes| render(decoder, &bytes, request))
            .transpose()
    });
    match cached {
        Ok(Some(image)) => return Ok(image),
        Err(error) if offline => {
            return Err(error.context("Offline image cache"));
        }
        _ => {}
    }
    if request.key()?.is_none() {
        return Ok(None);
    }
    ensure!(!offline, "Image unavailable in offline cache");
    let client = client.get_or_insert_with(http::Client::new);
    let bytes = client.download(request)?;
    let Some((key, mut image)) = render(decoder, &bytes, request)? else {
        return Ok(None);
    };
    image.cache_warning = cache::write(directory, &key.url, &bytes)
        .err()
        .map(|error| error.to_string());
    Ok(Some((key, image)))
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
mod tests;

impl Drop for Worker {
    fn drop(&mut self) {
        for request in &self.jobs {
            let _ = request.retarget(None);
        }
    }
}
