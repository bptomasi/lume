//! Flush consumes ordered results even after a failed admitted preference edit.

use super::*;
use std::time::Duration;

fn worker(pending: usize) -> (Worker, mpsc::SyncSender<Result<Completed>>) {
    let (requests, _receiver) = mpsc::sync_channel(1);
    let (sender, results) = mpsc::sync_channel(1);
    (
        Worker {
            requests,
            results,
            pending,
            flush_failure: None,
        },
        sender,
    )
}

fn completed() -> Result<Completed> {
    Ok(Completed {
        update: crate::model::RecommendationUpdate {
            total: 0,
            processed: 0,
            remaining: false,
        },
        page: None,
    })
}

#[test]
fn flush_drains_bounded_results_after_an_error() -> Result<()> {
    let (mut worker, sender) = worker(3);
    sender.send(Err(anyhow!("Preference write failed")))?;
    let producer = std::thread::spawn(move || -> Result<()> {
        sender.send(completed())?;
        sender.send(completed())?;
        Ok(())
    });
    let result = worker.flush(Instant::now() + Duration::from_secs(3));
    assert!(result.is_err());
    assert_eq!(worker.pending, 0);
    assert!(worker.poll().is_none());
    producer
        .join()
        .map_err(|_| anyhow!("Result producer stopped"))??;
    Ok(())
}

#[test]
fn an_expired_flush_retains_outstanding_results_for_retry() -> Result<()> {
    let (mut worker, sender) = worker(1);
    assert!(worker.flush(Instant::now()).is_err());
    assert_eq!(worker.pending, 1);
    sender.send(completed())?;
    worker.flush(Instant::now() + Duration::from_secs(3))?;
    assert!(!worker.pending());
    Ok(())
}

#[test]
fn disconnecting_with_unacknowledged_work_never_succeeds() {
    let (mut worker, sender) = worker(1);
    drop(sender);
    for _ in 0..2 {
        assert!(worker.flush(Instant::now()).is_err());
        assert_eq!(worker.pending, 1);
    }
    // Ordinary UI polling still acknowledges disconnection in one result.
    assert!(matches!(worker.poll(), Some(Err(_))));
    assert!(!worker.pending());
    assert!(worker.poll().is_none());
    assert!(worker.flush(Instant::now()).is_ok());
}

#[test]
fn a_flush_retry_reports_a_failure_consumed_before_timeout() -> Result<()> {
    let (mut worker, sender) = worker(2);
    sender.send(Err(anyhow!("First preference failed")))?;
    assert!(worker.flush(Instant::now()).is_err());
    assert_eq!(worker.pending, 1);
    sender.send(completed())?;
    let error = worker
        .flush(Instant::now())
        .err()
        .ok_or_else(|| anyhow!("Prior preference failure was lost"))?;
    assert_eq!(error.to_string(), "First preference failed");
    assert!(!worker.pending());
    worker.flush(Instant::now())?;
    assert!(worker.poll().is_none());
    Ok(())
}

#[test]
fn ordinary_poll_reports_a_retained_flush_failure_once() -> Result<()> {
    let (mut worker, sender) = worker(2);
    sender.send(Err(anyhow!("First preference failed")))?;
    assert!(worker.flush(Instant::now()).is_err());
    assert_eq!(worker.pending, 1);
    let error = worker
        .poll()
        .context("Missing retained preference failure")?
        .err()
        .context("Expected retained preference failure")?;
    assert_eq!(error.to_string(), "First preference failed");
    assert_eq!(worker.pending, 1);
    assert!(worker.poll().is_none());
    sender.send(completed())?;
    assert!(worker.poll().context("Missing remaining result")?.is_ok());
    assert!(!worker.pending());
    worker.flush(Instant::now())?;
    Ok(())
}
