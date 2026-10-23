//! A dedicated, bounded lane keeps filesystem and definition scans out of input.

use std::{
    cell::Cell,
    path::PathBuf,
    sync::{Arc, OnceLock, mpsc},
    thread,
    time::Duration,
};

use anyhow::Result;

mod state;

use super::Ui;

/// Complete publication snapshot; catalog fingerprints bound UI refreshes.
#[derive(Clone)]
pub struct Update {
    /// Local preference generation observed in the publication transaction.
    pub appearance_revision: u64,
    /// Complete appearance, permitting safe coalescing of notifications.
    pub ui: Ui,
    /// Complete named workspace definitions, excluding undo history.
    pub workspaces:
        std::collections::BTreeMap<String, crate::model::PanelLayout>,
    /// Content fingerprint of portable catalog definitions, excluding appearance.
    pub catalog_fingerprint: [u8; 32],
    /// Validated current file text for the discoverable configuration viewer.
    pub document: String,
    /// Prepared credential-safe view; absent when the document has no inline keys.
    pub redacted_document: Option<String>,
    /// Canonical effective configuration path.
    pub path: PathBuf,
    /// Cached scheduling preferences; article scans stay on the maintenance lane.
    pub retention: super::retention::Retention,
    /// Cached automatic-backup schedule; snapshots stay on maintenance workers.
    pub backups: super::backups::Backups,
}

/// Bounded reconciliation notifications; pending edits themselves live in SQLite.
pub struct Worker {
    events: mpsc::Receiver<Result<Update>>,
    stop: mpsc::SyncSender<Request>,
    thread: Option<thread::JoinHandle<Result<()>>>,
    failure: Arc<OnceLock<String>>,
    disconnected: Cell<bool>,
}

enum Request {
    Reload,
    Stop,
}

impl Worker {
    pub(super) fn start(path: PathBuf, database: PathBuf) -> Result<Self> {
        let (send, events) = mpsc::sync_channel(2);
        let (stop, receive) = mpsc::sync_channel(1);
        let failure = Arc::new(OnceLock::new());
        let reported = Arc::clone(&failure);
        let task = move || {
            let result = run(path, database, send.clone(), receive);
            if let Err(error) = &result {
                let _ = reported.set(format!("{error:#}"));
            }
            drop(send);
            result
        };
        let thread = thread::Builder::new()
            .name("lume-configuration".into())
            .spawn(task)?;
        Ok(Self {
            events,
            stop,
            thread: Some(thread),
            failure,
            disconnected: Cell::new(false),
        })
    }

    /// Take a completed notification without blocking navigation.
    pub fn poll(&self) -> Option<Result<Update>> {
        match self.events.try_recv() {
            Ok(update) => Some(update),
            Err(mpsc::TryRecvError::Disconnected)
                if !self.disconnected.replace(true) =>
            {
                let message = self
                    .failure
                    .get()
                    .map_or("connection closed unexpectedly", String::as_str);
                Some(Err(anyhow::anyhow!(
                    "Configuration worker stopped: {message}"
                )))
            }
            _ => None,
        }
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

#[cfg(test)]
mod tests {
    use super::{Update, Worker};
    use anyhow::{Context, Result};
    use std::{
        cell::Cell,
        sync::{Arc, OnceLock, mpsc},
    };

    #[test]
    fn fatal_failure_survives_full_notifications_and_is_reported_once()
    -> Result<()> {
        let (send, events) = mpsc::sync_channel(2);
        for name in ["First", "Second"] {
            send.send(Ok(notification(name)))?;
        }
        let failure =
            Arc::new(OnceLock::from("Storage became unavailable".to_owned()));
        drop(send);
        let (stop, _requests) = mpsc::sync_channel(1);
        let worker = Worker {
            events,
            stop,
            thread: None,
            failure,
            disconnected: Cell::new(false),
        };
        assert_eq!(
            worker.poll().context("First update missing")??.document,
            "First"
        );
        assert_eq!(
            worker.poll().context("Second update missing")??.document,
            "Second"
        );
        let error = worker
            .poll()
            .context("Failure was lost")?
            .err()
            .context("Worker failure was reported as success")?;
        assert!(error.to_string().contains("Storage became unavailable"));
        assert!(worker.poll().is_none());
        Ok(())
    }
    fn notification(name: &str) -> Update {
        Update {
            appearance_revision: 0,
            retention: Default::default(),
            backups: Default::default(),
            ui: Default::default(),
            workspaces: Default::default(),
            catalog_fingerprint: [0; 32],
            document: name.into(),
            redacted_document: None,
            path: "config.toml".into(),
        }
    }
}
