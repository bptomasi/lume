//! Controlled queues qualify replacement without network or timing assumptions.
use super::*;
use anyhow::Result;

#[test]
#[cfg(all(feature = "svg", feature = "sixel"))]
fn production_image_measurements_are_opt_in_and_separate_real_stages()
-> Result<()> {
    use crate::media::{ImageDetail, ImageProtocol, profiling::Step};
    let directory = tempfile::tempdir()?;
    let mut target = key("https://a.test/profile", 40);
    target.detail = Some(ImageDetail {
        zoom: 1,
        pan: [0; 2],
        protocol: ImageProtocol::Sixel,
        cell_pixels: (2, 4),
        slice: None,
    });
    let bytes =
        include_bytes!("../../../tests/fixtures/images/request-path.svg");
    let mut request = Request::new(target, Some(Arc::from(bytes.as_slice())));
    let decoder = Mutex::new(Decoder::default());
    let (result, profile) =
        load_measured(directory.path(), true, &mut None, &decoder, &request);
    assert!(result?.is_some());
    assert!(profile.is_none());
    request.measured = true;
    let (result, profile) =
        load_measured(directory.path(), true, &mut None, &decoder, &request);
    assert!(result?.is_some());
    let sample = profile.context("Image profile missing")?;
    for stage in [Step::Decode, Step::Resize, Step::Exact, Step::Encode] {
        assert!(sample.calls[stage as usize] > 0);
    }
    assert_eq!(sample.calls[Step::Pipeline as usize], 0);
    assert!(sample.total_ns >= sample.stages_ns.iter().sum());
    Ok(())
}
fn key(url: &str, width: u16) -> ImageKey {
    ImageKey {
        url: url.into(),
        width,
        height: 12,
        background: [0; 3],
        detail: None,
    }
}

fn worker() -> (
    Worker,
    mpsc::Receiver<Arc<Request>>,
    mpsc::SyncSender<Completion>,
) {
    let (requests, receiver) = mpsc::sync_channel(2);
    let (sender, results) = mpsc::sync_channel(2);
    (
        Worker {
            requests,
            results,
            jobs: Vec::new(),
        },
        receiver,
        sender,
    )
}

#[test]
fn disconnected_worker_reports_failure_instead_of_staying_pending() {
    let (worker, _receiver, sender) = worker();
    assert!(matches!(worker.poll(), Ok(None)));
    drop(sender);
    assert!(worker.poll().is_err());
}

#[test]
fn crop_changes_share_one_source_job_and_full_queues_are_retryable()
-> Result<()> {
    let (mut worker, receiver, _sender) = worker();
    let first = key("https://a.test/image", 40);
    let latest = key("https://a.test/image", 80);
    assert!(worker.submit(first, None, false)?);
    assert!(worker.submit(latest.clone(), None, false)?);
    assert!(worker.submit(key("https://b.test/image", 40), None, false)?);
    assert!(!worker.submit(key("https://c.test/image", 40), None, false)?);
    let request = receiver.try_recv()?;
    assert_eq!(request.key()?, Some(latest));
    assert!(receiver.try_recv().is_ok());
    assert!(receiver.try_recv().is_err());
    worker.retain(&[])?;
    assert!(request.key()?.is_none());
    Ok(())
}

#[test]
fn an_obsolete_queued_source_stops_before_disk_http_and_decode() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let request =
        Request::new(key("invalid source must not be opened", 40), None);
    request.retarget(None)?;
    let mut client = None;
    let result = load(
        directory.path(),
        false,
        &mut client,
        &Mutex::new(Decoder::default()),
        &request,
    )?;
    assert!(result.is_none());
    assert!(client.is_none());
    assert!(std::fs::read_dir(directory.path())?.next().is_none());
    assert!(!request.finish()?);
    assert!(!request.retarget(Some(key("https://a.test/image", 80)))?);
    Ok(())
}

#[test]
fn the_latest_crop_is_captured_after_waiting_for_the_shared_decoder()
-> Result<()> {
    let decoder = Arc::new(Mutex::new(Decoder::default()));
    let guard = decoder
        .lock()
        .map_err(|_| anyhow::anyhow!("Decoder lock"))?;
    let request = Arc::new(Request::new(key("https://a.test/image", 40), None));
    let (source, pending) = (decoder.clone(), request.clone());
    let thread = thread::spawn(move || render(&source, &raster()?, &pending));
    let latest = key("https://a.test/image", 80);
    request.retarget(Some(latest.clone()))?;
    drop(guard);
    let (actual, _) = thread
        .join()
        .map_err(|_| anyhow::anyhow!("Render thread"))??
        .context("Latest crop was discarded")?;
    assert_eq!(actual, latest);
    Ok(())
}

#[test]
fn an_encoding_replacement_reuses_source_bytes_and_delivers_only_the_latest_crop()
-> Result<()> {
    let request = Request::new(key("https://a.test/image", 40), None);
    let latest = key("https://a.test/image", 80);
    let mut attempts = 0;
    let result = render_with(
        &Mutex::new(Decoder::default()),
        &raster()?,
        &request,
        |image, dimensions, key, cancel| {
            attempts += 1;
            if attempts == 1 {
                request.retarget(Some(latest.clone()))?;
                return Err(anyhow::anyhow!("Obsolete encoder interrupted"));
            }
            super::super::decode::prepare_cancellable(
                image, dimensions, key, cancel,
            )
        },
    )?
    .context("Replacement result")?;
    assert_eq!(attempts, 2);
    assert_eq!(result.0, latest);
    let (_, cancel) = request.snapshot()?.context("Current snapshot")?;
    request.retarget(Some(latest))?;
    assert!(!cancel.is_cancelled(), "Identical retarget restarted work");
    request.retarget(None)?;
    assert!(cancel.is_cancelled());
    Ok(())
}

fn raster() -> Result<Vec<u8>> {
    let mut bytes = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(
        2,
        2,
        image::Rgb([255, 0, 0]),
    ))
    .write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

#[test]
#[cfg(not(feature = "svg"))]
fn unsupported_cached_svg_never_constructs_a_network_client() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let key = key("https://example.org/image.svg", 40);
    cache::write(
        directory.path(),
        &key.url,
        include_bytes!("../../../tests/fixtures/images/request-path.svg"),
    )?;
    let request = Request::new(key, None);
    let mut client = None;
    let error = load(
        directory.path(),
        false,
        &mut client,
        &Mutex::new(Decoder::default()),
        &request,
    )
    .err()
    .context("SVG capability error")?;
    assert!(error.is::<super::super::UnavailableSvg>());
    assert!(client.is_none());
    Ok(())
}
