//! A dedicated, bounded lane keeps filesystem and definition scans out of input.

use std::{path::PathBuf, sync::mpsc, thread, time::Duration};

use anyhow::Result;

mod state;

use super::Ui;

/// Complete publication snapshot; catalog fingerprints bound UI refreshes.
#[derive(Clone)]
pub struct Update {
    /// Complete appearance, permitting safe coalescing of notifications.
    pub ui: Ui,
    /// Complete named workspace definitions, excluding undo history.
    pub workspaces:
        std::collections::BTreeMap<String, crate::model::PanelLayout>,
    /// Content fingerprint of portable catalog definitions, excluding appearance.
    pub catalog_fingerprint: [u8; 32],
    /// Validated current file text for the discoverable configuration viewer.
    pub document: String,
    /// Canonical effective configuration path.
    pub path: PathBuf,
}

/// Bounded reconciliation notifications; pending edits themselves live in SQLite.
pub struct Worker {
    events: mpsc::Receiver<Result<Update>>,
    stop: mpsc::SyncSender<Request>,
    thread: Option<thread::JoinHandle<Result<()>>>,
}

enum Request {
    Reload,
    Stop,
}

impl Worker {
    pub(super) fn start(path: PathBuf, database: PathBuf) -> Result<Self> {
        let (send, events) = mpsc::sync_channel(2);
        let (stop, receive) = mpsc::sync_channel(1);
        let thread = thread::Builder::new()
            .name("lume-configuration".into())
            .spawn(move || {
            let result = run(path, database, send.clone(), receive);
            if let Err(error) = &result {
                let _ = send.try_send(Err(anyhow::anyhow!(
                    "Configuration worker stopped: {error:#}"
                )));
            }
            result
        })?;
        Ok(Self {
            events,
            stop,
            thread: Some(thread),
        })
    }

    /// Take a completed notification without blocking navigation.
    pub fn poll(&self) -> Option<Result<Update>> {
        self.events.try_recv().ok()
    }

    /// Request immediate reconciliation; the bounded queue rejects overload.
    ///
    /// # Errors
    /// Reports an outstanding request or a stopped configuration worker.
    pub fn reload(&self) -> Result<()> {
        self.stop.try_send(Request::Reload).map_err(|_| {
            anyhow::anyhow!(
                "Configuration request already pending or worker stopped"
            )
        })
    }

    /// Drain durable edits after application writers have stopped.
    ///
    /// # Errors
    /// Reports publication/conflict failures or an unexpectedly stopped worker.
    pub fn finish(mut self) -> Result<()> {
        let _ = self.stop.send(Request::Stop);
        let Some(thread) = self.thread.take() else {
            return Ok(());
        };
        thread
            .join()
            .map_err(|_| anyhow::anyhow!("Configuration worker stopped"))?
    }
}

fn run(
    path: PathBuf,
    database: PathBuf,
    send: mpsc::SyncSender<Result<Update>>,
    stop: mpsc::Receiver<Request>,
) -> Result<()> {
    let mut watch = state::Watch::open(path, &database)?;
    let mut notifications = state::Notifications::default();
    loop {
        if !notifications.deliver(&send) {
            return Ok(());
        }
        let forced = match stop.recv_timeout(Duration::from_millis(250)) {
            Ok(Request::Reload) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => false,
            _ => return watch.finish(),
        };
        if let Some(result) = watch.tick(forced)? {
            notifications.record(result);
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let _ = self.stop.send(Request::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
