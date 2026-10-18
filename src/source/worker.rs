//! Bounded worker lifecycle; cancellation is checked between network requests.

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread,
};

use anyhow::{Context, Result};

/// A worker result; articles themselves remain in the database.
#[derive(Debug)]
pub enum Progress {
    /// Incremental collection progress; does not replace current reader text.
    Updated {
        /// Completed subscriptions in this pass.
        completed: usize,
        /// Eligible subscriptions, excluding durable retry deadlines.
        total: usize,
    },
    /// A collection pass completed, possibly with individual feed failures.
    Finished {
        /// Number of feeds visited.
        feeds: usize,
        /// Number of feeds that failed.
        failures: usize,
    },
    /// The worker could not access storage.
    Failed(String),
    /// Advertised feed choices from a website.
    Discovered(Vec<super::discovery::Candidate>),
}

enum Request {
    Refresh,
    Discover(String),
}

/// A single refresh worker with a bounded command and result queue.
pub struct Worker {
    commands: SyncSender<Request>,
    results: Receiver<Progress>,
    stopped: Arc<AtomicBool>,
}

impl Worker {
    /// Start a worker; dropping it requests cancellation between feeds.
    ///
    /// # Errors
    /// Returns an error if the operating system cannot create the thread.
    pub fn start(path: PathBuf) -> Result<Self> {
        let (commands, requests) = mpsc::sync_channel(1);
        let (results, receiver) = mpsc::sync_channel(1);
        let stopped = Arc::new(AtomicBool::new(false));
        let cancellation = Arc::clone(&stopped);
        thread::Builder::new()
            .name("lume-collector".into())
            .spawn(move || run(path, requests, results, cancellation))?;
        Ok(Self {
            commands,
            results: receiver,
            stopped,
        })
    }

    /// Submit a refresh without blocking the UI on a full queue.
    ///
    /// # Errors
    /// Returns a busy or disconnected-worker error.
    pub fn refresh(&self) -> Result<()> {
        self.commands
            .try_send(Request::Refresh)
            .context("Collector is busy or stopped")
    }

    /// Submit a discovery request without blocking the UI.
    ///
    /// # Errors
    /// Returns a full-queue or disconnected-worker error.
    pub fn discover(&self, url: String) -> Result<()> {
        self.commands
            .try_send(Request::Discover(url))
            .context("Collector is busy or stopped")
    }

    /// Receive a completion if one is available, without waiting.
    pub fn poll(&self) -> Option<Progress> {
        self.results.try_recv().ok()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.stopped.store(true, Ordering::Relaxed);
    }
}

pub(super) fn collect_once(path: &std::path::Path) -> Result<(usize, usize)> {
    let progress = super::scheduler::refresh(
        path,
        &super::agent(),
        &AtomicBool::new(false),
        |_, _| {},
    )?;
    match progress {
        Progress::Finished { feeds, failures } => Ok((feeds, failures)),
        Progress::Updated { .. } => anyhow::bail!("Unexpected progress result"),
        Progress::Failed(message) => anyhow::bail!("{message}"),
        Progress::Discovered(_) => anyhow::bail!("Unexpected discovery result"),
    }
}

fn run(
    path: PathBuf,
    requests: Receiver<Request>,
    results: SyncSender<Progress>,
    stopped: Arc<AtomicBool>,
) {
    let client = super::agent();
    while let Ok(request) = requests.recv() {
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        let result = match request {
            Request::Refresh => super::scheduler::refresh(
                &path,
                &client,
                &stopped,
                |completed, total| {
                    let _ = results
                        .try_send(Progress::Updated { completed, total });
                },
            ),
            Request::Discover(url) => {
                super::discovery::discover(&url).map(Progress::Discovered)
            }
        };
        let progress =
            result.unwrap_or_else(|error| Progress::Failed(error.to_string()));
        if results.send(progress).is_err() {
            break;
        }
    }
}
