//! A scoped SQLite progress callback belongs only to disposable read work.
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use super::Store;
use crate::cancellation::Cancellation;
use anyhow::{Context, Result};

struct Progress<'a> {
    store: &'a mut Store,
    active: Arc<AtomicBool>,
    finished: bool,
}

impl Progress<'_> {
    fn finish(mut self) -> Result<()> {
        self.active.store(false, Ordering::Release);
        self.finished = true;
        self.store
            .connection
            .progress_handler(0, None::<fn() -> bool>)
            .context("Could not remove the SQLite progress callback")
    }
}

impl Drop for Progress<'_> {
    fn drop(&mut self) {
        // Even unwinding or a failed unregister leaves an inert callback.
        self.active.store(false, Ordering::Release);
        if !self.finished {
            // Unwinding cannot return a cleanup error. Normal returns always
            // use finish(), which propagates it with the query's error context.
            let _ = self
                .store
                .connection
                .progress_handler(0, None::<fn() -> bool>);
        }
    }
}

impl Store {
    pub(crate) fn read_cancellable<T>(
        &mut self,
        cancel: Cancellation,
        read: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        cancel.check()?;
        let progress = cancel.clone();
        let active = Arc::new(AtomicBool::new(true));
        let enabled = active.clone();
        self.connection.progress_handler(
            2048,
            Some(move || {
                enabled.load(Ordering::Acquire) && progress.is_cancelled()
            }),
        )?;
        let guard = Progress {
            store: self,
            active,
            finished: false,
        };
        // Query owners choose their own snapshot/transaction boundaries.
        let result = read(guard.store);
        if let Err(cleanup) = guard.finish() {
            return Err(match result {
                Ok(_) => cleanup,
                Err(error) => error.context(format!("{cleanup:#}")),
            });
        }
        cancel.check()?;
        result
    }
}

#[cfg(test)]
mod tests;
