//! Channel-controlled flush admission, cancellation and failure cases.

use super::*;

fn writer(
    bound: usize,
) -> (SessionWriter, mpsc::Receiver<Write>, mpsc::SyncSender<()>) {
    let (requests, receiver) = mpsc::sync_channel(bound);
    let (_errors, errors) = mpsc::sync_channel(1);
    let (available, capacity) = mpsc::sync_channel(1);
    let writer = SessionWriter {
        requests,
        errors,
        pending: Arc::new(AtomicUsize::new(0)),
        failures: Arc::new(AtomicUsize::new(0)),
        capacity,
    };
    (writer, receiver, available)
}

fn position() -> Write {
    Write::Progress(Progress {
        article: 7,
        source: ContentSource::Feed,
        revision: 0,
        anchor: 42,
    })
}

#[test]
fn a_full_queue_admits_the_barrier_after_the_earlier_write() -> Result<()> {
    let (writer, receiver, available) = writer(1);
    writer.submit(position())?;
    let pending = writer.pending.clone();
    let thread = std::thread::spawn(move || writer.flush());
    assert!(matches!(
        receiver.recv_timeout(Duration::from_secs(3))?,
        Write::Progress(Progress { anchor: 42, .. })
    ));
    pending.fetch_sub(1, Ordering::AcqRel);
    available.send(())?;
    let Write::Flush(done) = receiver.recv_timeout(Duration::from_secs(3))?
    else {
        anyhow::bail!("Expected the ordered flush barrier");
    };
    done.send(())?;
    thread
        .join()
        .map_err(|_| anyhow!("Flush test thread stopped"))??;
    Ok(())
}

#[test]
fn an_expired_full_queue_keeps_the_admitted_write() -> Result<()> {
    let (writer, receiver, _available) = writer(1);
    writer.submit(position())?;
    let (done, _completion) = mpsc::sync_channel(1);
    assert!(
        admit_barrier(
            &writer.requests,
            &writer.capacity,
            Write::Flush(done),
            Instant::now(),
        )
        .is_err()
    );
    assert!(writer.pending());
    assert!(matches!(receiver.try_recv()?, Write::Progress(_)));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    Ok(())
}

#[test]
fn a_closed_capacity_notification_cannot_report_flush_success() -> Result<()> {
    let (writer, _receiver, available) = writer(1);
    writer.submit(position())?;
    drop(available);
    assert!(writer.flush().is_err());
    assert!(writer.pending());
    Ok(())
}

#[test]
fn a_cancelled_barrier_cannot_report_flush_success() -> Result<()> {
    let (writer, receiver, _available) = writer(1);
    let thread = std::thread::spawn(move || writer.flush());
    let Write::Flush(done) = receiver.recv_timeout(Duration::from_secs(3))?
    else {
        anyhow::bail!("Expected flush barrier");
    };
    drop(done);
    assert!(
        thread
            .join()
            .map_err(|_| anyhow!("Flush thread stopped"))?
            .is_err()
    );
    Ok(())
}

#[test]
fn a_completed_barrier_still_reports_unresolved_writes() -> Result<()> {
    let (writer, receiver, _available) = writer(1);
    writer.failures.store(1, Ordering::Release);
    let thread = std::thread::spawn(move || writer.flush());
    let Write::Flush(done) = receiver.recv_timeout(Duration::from_secs(3))?
    else {
        anyhow::bail!("Expected flush barrier");
    };
    done.send(())?;
    let result = thread.join().map_err(|_| anyhow!("Flush thread stopped"))?;
    assert!(result.is_err());
    Ok(())
}

fn start_worker(
    writer: &mut SessionWriter,
    receiver: mpsc::Receiver<Write>,
    available: mpsc::SyncSender<()>,
    store: Store,
) -> std::thread::JoinHandle<()> {
    let (errors, outcomes) = mpsc::sync_channel(WRITE_QUEUE);
    writer.errors = outcomes;
    let pending = writer.pending.clone();
    let failures = writer.failures.clone();
    std::thread::spawn(move || {
        run(store, receiver, errors, pending, failures, available);
    })
}

#[test]
fn a_full_queue_durably_drains_changing_layout_writes() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("full-queue.sqlite3");
    let store = Store::open(&path)?;
    let (mut writer, receiver, available) = writer(WRITE_QUEUE);
    for page_offset in 0..WRITE_QUEUE {
        let workspace = Workspace {
            page_offset,
            ..Workspace::default()
        };
        writer.layout(&workspace, &LayoutHistory::new(&workspace))?;
    }
    let workspace = Workspace::default();
    assert!(
        writer
            .layout(&workspace, &LayoutHistory::new(&workspace))
            .is_err()
    );
    assert_eq!(writer.pending.load(Ordering::Acquire), WRITE_QUEUE);
    let thread = start_worker(&mut writer, receiver, available, store);
    writer.flush()?;
    assert!(!writer.pending());
    assert!(!writer.failed());
    assert_eq!(
        Store::open(&path)?.workspace()?.page_offset,
        WRITE_QUEUE - 1
    );
    drop(writer);
    thread
        .join()
        .map_err(|_| anyhow!("Session worker stopped"))?;
    Ok(())
}

#[test]
fn a_dropped_barrier_receiver_does_not_cancel_later_durable_writes()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("cancelled-barrier.sqlite3");
    let store = Store::open(&path)?;
    let (mut writer, receiver, available) = writer(2);
    drop(writer.barrier()?.context("Barrier not admitted")?);
    let workspace = Workspace {
        page_offset: 17,
        ..Workspace::default()
    };
    writer.layout(&workspace, &LayoutHistory::new(&workspace))?;
    let thread = start_worker(&mut writer, receiver, available, store);
    writer.flush()?;
    assert_eq!(Store::open(&path)?.workspace()?.page_offset, 17);
    assert!(!writer.pending());
    drop(writer);
    thread
        .join()
        .map_err(|_| anyhow!("Session worker stopped"))?;
    Ok(())
}

#[test]
fn an_expired_completion_wait_does_not_claim_the_barrier_finished() -> Result<()>
{
    let (writer, receiver, _available) = writer(1);
    assert!(writer.flush_before(Instant::now()).is_err());
    let Write::Flush(done) = receiver.try_recv()? else {
        anyhow::bail!("Expected admitted barrier after completion timeout");
    };
    // Timing out only abandons this acknowledgement, never an earlier write.
    assert!(done.send(()).is_err());
    Ok(())
}

#[test]
fn a_disconnected_request_queue_cannot_report_flush_success() {
    let (writer, receiver, _available) = writer(1);
    drop(receiver);
    assert!(writer.flush().is_err());
}

#[test]
fn a_stale_capacity_wake_does_not_admit_a_barrier_or_restart_its_budget()
-> Result<()> {
    let (writer, receiver, available) = writer(1);
    writer.submit(position())?;
    available.send(())?;
    let deadline = Instant::now() + Duration::from_millis(10);
    assert!(writer.flush_before(deadline).is_err());
    assert!(matches!(receiver.try_recv()?, Write::Progress(_)));
    assert!(matches!(
        receiver.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert!(writer.pending());
    Ok(())
}
