//! Cooperative cancellation never owns or supersedes a durable mutation.
use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
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

/// Retention planning cancellation is recognized separately from a completed,
/// partially applied batch report and from superseded reader requests.
#[derive(Debug)]
pub(crate) struct RetentionCanceled;

impl std::fmt::Display for RetentionCanceled {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("Retention planning canceled")
    }
}

impl std::error::Error for RetentionCanceled {}

pub(crate) fn check_retention(cancel: &AtomicBool) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(RetentionCanceled.into());
    }
    Ok(())
}

/// Backup cancellation must prevent publication, including after copying pages.
#[derive(Debug)]
pub(crate) struct BackupCanceled;

impl std::fmt::Display for BackupCanceled {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        output.write_str("Backup cancelled before publication")
    }
}

impl std::error::Error for BackupCanceled {}

pub(crate) fn check_backup(cancel: &AtomicBool) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(BackupCanceled.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maintenance_cancellation_preserves_distinct_error_contracts()
    -> anyhow::Result<()> {
        let cancel = AtomicBool::new(false);
        check_backup(&cancel)?;
        check_retention(&cancel)?;
        cancel.store(true, Ordering::Relaxed);
        let backup = check_backup(&cancel).err().ok_or_else(|| {
            anyhow::anyhow!("Backup cancellation was ignored")
        })?;
        let retention = check_retention(&cancel).err().ok_or_else(|| {
            anyhow::anyhow!("Retention cancellation was ignored")
        })?;
        assert!(backup.is::<BackupCanceled>());
        assert!(!backup.is::<RetentionCanceled>());
        assert_eq!(backup.to_string(), "Backup cancelled before publication");
        assert!(retention.is::<RetentionCanceled>());
        assert!(!retention.is::<Canceled>());
        assert_eq!(retention.to_string(), "Retention planning canceled");
        Ok(())
    }
}
