//! Cooperative cancellation never owns or supersedes a durable mutation.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

#[derive(Clone)]
pub(crate) enum Cancellation {
    Generation(Arc<AtomicU64>, u64),
}

impl Cancellation {
    pub(crate) fn is_cancelled(&self) -> bool {
        match self {
            Self::Generation(current, captured) => {
                current.load(Ordering::Acquire) != *captured
            }
        }
    }

    pub(crate) fn check(&self) -> anyhow::Result<()> {
        if self.is_cancelled() {
            return Err(Canceled.into());
        }
        Ok(())
    }
}

#[derive(Debug)]
pub(crate) struct Canceled;

impl std::fmt::Display for Canceled {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Operation superseded or cancelled")
    }
}

impl std::error::Error for Canceled {}

#[cfg(test)]
pub(crate) fn test_token() -> (Arc<AtomicU64>, Cancellation) {
    let current = Arc::new(AtomicU64::new(0));
    (current.clone(), Cancellation::Generation(current, 0))
}
