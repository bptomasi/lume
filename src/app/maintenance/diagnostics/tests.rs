//! A blocked durable lane cannot reject or receive a storage cancellation.
use super::*;
use crate::app::{
    Mode,
    maintenance::{Maintenance, Request, Update},
};
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
    app.maintenance = Some(worker);
    app.submit_maintenance(Request::StorageDetails(Unit::Bytes))?;
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    while app
        .storage_diagnostics
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
        matches!(&app.mode, Mode::Document(view) if view.title == "Storage · Esc closes")
    );
    app.submit_maintenance(Request::Storage(Unit::MiB))?;
    app.cancel_storage_diagnostic();
    assert!(!cancel.load(Ordering::Relaxed));
    assert!(app.durable_maintenance_pending());
    assert!(
        !app.storage_diagnostics
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
    app.maintenance = Some(worker);
    app.cancel_storage_diagnostic();
    assert!(cancel.load(Ordering::Relaxed));
    assert!(app.durable_maintenance_pending());
    Ok(())
}
