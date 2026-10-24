//! A scoped SQLite progress callback belongs only to disposable read work.
use super::Store;
use crate::cancellation::Cancellation;
use anyhow::Result;

struct Progress<'a>(&'a rusqlite::Connection);

impl Drop for Progress<'_> {
    fn drop(&mut self) {
        self.0.progress_handler(0, None::<fn() -> bool>);
    }
}

impl Store {
    pub(crate) fn read_cancellable<T>(
        &self,
        cancel: Cancellation,
        read: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        cancel.check()?;
        let progress = cancel.clone();
        self.connection
            .progress_handler(2048, Some(move || progress.is_cancelled()));
        let guard = Progress(&self.connection);
        let result = self.read_snapshot(read);
        drop(guard);
        cancel.check()?;
        result
    }
}

#[cfg(test)]
mod tests;
