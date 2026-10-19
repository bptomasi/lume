//! A watchdog closes a blocked pipe by terminating its owned helper process.
//!
//! No control lock is held during writes. The watcher never touches the writer,
//! so it can kill an unresponsive reader and release the worker's blocked write.

use std::{
    process::{Child, ChildStdin},
    sync::{Arc, Condvar, Mutex, MutexGuard},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};

struct State {
    child: Child,
    deadline: Option<Instant>,
    stopped: bool,
    timed_out: bool,
}

struct Shared {
    state: Mutex<State>,
    changed: Condvar,
}

pub(super) struct Process {
    shared: Arc<Shared>,
    watcher: Option<JoinHandle<()>>,
}

impl Process {
    pub(super) fn start(child: Child) -> Result<Self> {
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                child,
                deadline: None,
                stopped: false,
                timed_out: false,
            }),
            changed: Condvar::new(),
        });
        let monitor = shared.clone();
        let mut process = Self {
            shared,
            watcher: None,
        };
        process.watcher = Some(
            thread::Builder::new()
                .name("lume-overlay-watchdog".into())
                .spawn(move || watch(monitor))?,
        );
        Ok(process)
    }

    pub(super) fn take_stdin(&self) -> Result<ChildStdin> {
        self.state()?
            .child
            .stdin
            .take()
            .context("Image helper stdin unavailable")
    }

    pub(super) fn running(&self) -> Result<()> {
        let mut state = self.state()?;
        ensure!(!state.timed_out, "Image helper stopped accepting frames");
        ensure!(
            state.child.try_wait()?.is_none(),
            "Image helper exited; verify its installation and display backend"
        );
        Ok(())
    }

    pub(super) fn begin_write(&self) -> Result<()> {
        self.running()?;
        self.state()?.deadline =
            Some(Instant::now() + Duration::from_millis(200));
        self.shared.changed.notify_one();
        Ok(())
    }

    pub(super) fn end_write(&self) -> Result<()> {
        let mut state = self.state()?;
        state.deadline = None;
        self.shared.changed.notify_one();
        ensure!(!state.timed_out, "Image helper stopped accepting frames");
        Ok(())
    }

    fn state(&self) -> Result<MutexGuard<'_, State>> {
        self.shared
            .state
            .lock()
            .map_err(|_| anyhow::anyhow!("Image helper control poisoned"))
    }
}

fn watch(shared: Arc<Shared>) {
    let Ok(mut state) = shared.state.lock() else {
        return;
    };
    while !state.stopped {
        if let Some(deadline) = state.deadline {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                state.timed_out = true;
                state.deadline = None;
                let _ = state.child.kill();
                continue;
            }
            let Ok((next, _)) = shared.changed.wait_timeout(state, remaining)
            else {
                return;
            };
            state = next;
        } else {
            let Ok(next) = shared.changed.wait(state) else {
                return;
            };
            state = next;
        }
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        {
            // Preserve cleanup authority even if another operation poisoned it.
            let mut state = match self.shared.state.lock() {
                Ok(state) => state,
                Err(poisoned) => poisoned.into_inner(),
            };
            state.stopped = true;
            let _ = state.child.kill();
            let _ = state.child.wait();
        }
        self.shared.changed.notify_one();
        if let Some(watcher) = self.watcher.take() {
            let _ = watcher.join();
        }
    }
}
