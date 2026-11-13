//! Worker admission and acknowledgments preserve independent storage reads.
use super::*;
use crate::app::{
    App, Mode,
    maintenance::{Maintenance, Request, Update},
};
use crate::store::Store;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
    mpsc,
};

#[test]
fn diagnostics_run_while_durable_work_is_busy_and_cancel_independently()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let (worker, _sender) = busy_worker(cancel.clone());
    app.maintenance.worker = Some(worker);
    app.submit_maintenance(Request::StorageDetails(Unit::Bytes))?;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app
        .maintenance
        .diagnostics
        .as_ref()
        .is_some_and(Diagnostics::pending)
    {
        app.poll_maintenance()?;
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Inspection blocked by maintenance"
        );
        std::thread::yield_now();
    }
    assert!(app.durable_maintenance_pending());
    assert!(
        matches!(app.interaction.mode(), Mode::Document(view) if view.title == "Storage · Esc closes")
    );
    app.submit_maintenance(Request::Storage(Unit::MiB))?;
    app.cancel_storage_diagnostic();
    assert!(!cancel.load(Ordering::Relaxed));
    assert!(app.durable_maintenance_pending());
    assert!(
        !app.maintenance
            .diagnostics
            .as_ref()
            .is_some_and(Diagnostics::pending)
    );
    Ok(())
}

fn busy_worker(
    cancel: Arc<AtomicBool>,
) -> (Maintenance, mpsc::SyncSender<Update>) {
    let (requests, _receiver) = mpsc::sync_channel(1);
    let (sender, results) = mpsc::sync_channel(2);
    (
        Maintenance {
            requests,
            results,
            cancel,
            pending: true,
            cleaning: false,
        },
        sender,
    )
}

#[test]
fn storage_cancel_still_stops_an_active_cache_cleanup() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    let cancel = Arc::new(AtomicBool::new(false));
    let (mut worker, _sender) = busy_worker(cancel.clone());
    worker.cleaning = true;
    app.maintenance.worker = Some(worker);
    app.cancel_storage_diagnostic();
    assert!(cancel.load(Ordering::Relaxed));
    assert!(app.durable_maintenance_pending());
    Ok(())
}

#[test]
fn failed_admission_preserves_cancellation_and_does_not_claim_work()
-> Result<()> {
    let cancel = Arc::new(AtomicBool::new(true));
    let (mut worker, _sender) = busy_worker(cancel.clone());
    worker.pending = false;
    let mut state = MaintenanceState {
        worker: Some(worker),
        ..MaintenanceState::default()
    };

    assert!(
        state
            .admit("unused.sqlite3".into(), Request::Resume)
            .is_err()
    );
    assert!(!state.pending());
    assert!(cancel.load(Ordering::Relaxed));
    assert!(!state.cancel_storage());
    Ok(())
}

#[test]
fn terminal_failure_releases_admission_and_is_delivered_once() -> Result<()> {
    let cancel = Arc::new(AtomicBool::new(false));
    let (mut worker, sender) = busy_worker(cancel);
    worker.cleaning = true;
    let mut state = MaintenanceState {
        worker: Some(worker),
        ..MaintenanceState::default()
    };
    sender.send(Update::Progress("One committed batch".into()))?;
    assert!(matches!(state.poll()?, Some(Update::Progress(_))));
    assert!(state.durable_pending());
    sender.send(Update::Finished(Err(anyhow::anyhow!("Disk failure"))))?;

    let Some(Update::Finished(result)) = state.poll()? else {
        anyhow::bail!("Terminal failure was not delivered");
    };
    assert_eq!(
        result.err().context("Failure was hidden")?.to_string(),
        "Disk failure"
    );
    assert!(!state.durable_pending());
    assert!(!state.cancel_storage());
    assert!(state.poll()?.is_none());
    Ok(())
}

#[test]
fn disconnected_cleanup_releases_its_slot_and_reports_once() -> Result<()> {
    let (mut worker, sender) = busy_worker(Arc::new(AtomicBool::new(false)));
    worker.cleaning = true;
    let mut state = MaintenanceState {
        worker: Some(worker),
        ..MaintenanceState::default()
    };
    drop(sender);

    assert!(state.poll().is_err());
    assert!(!state.durable_pending());
    assert!(!state.cancel_storage());
    assert!(state.poll()?.is_none());
    Ok(())
}
