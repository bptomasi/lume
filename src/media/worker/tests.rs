//! Controlled queues qualify replacement without network or timing assumptions.
use super::*;
use anyhow::Result;

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
    assert!(worker.submit(first, None)?);
    assert!(worker.submit(latest.clone(), None)?);
    assert!(worker.submit(key("https://b.test/image", 40), None)?);
    assert!(!worker.submit(key("https://c.test/image", 40), None)?);
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
    let thread = thread::spawn(move || {
        render(
            &source,
            include_bytes!("../../../tests/fixtures/images/request-path.svg"),
            &pending,
        )
    });
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
