//! File observation and bounded notification replacement have separate owners.

use std::{
    path::{Path, PathBuf},
    sync::mpsc,
};

use anyhow::Result;

use crate::store::Store;

use super::Update;

type Notice = Result<Update>;

pub(super) struct Watch {
    path: PathBuf,
    store: Store,
    observed: Option<(std::time::SystemTime, u64)>,
}

impl Watch {
    pub(super) fn open(path: PathBuf, database: &Path) -> Result<Self> {
        Ok(Self {
            // The file may have changed since Session::open validated it.
            observed: None,
            path,
            store: Store::open(database)?,
        })
    }

    pub(super) fn tick(&mut self, forced: bool) -> Result<Option<Notice>> {
        let pending = self.store.configuration_pending(&self.path)?;
        let observed = stamp(&self.path);
        if !forced
            && !pending
            && observed.is_some()
            && self.observed == observed
        {
            return Ok(None);
        }
        let result = self.store.reconcile_configuration(&self.path);
        if result.is_ok() {
            // Record the input observation, never a later unvalidated edit.
            // Our own atomic publication may cause one follow-up check.
            self.observed = observed;
        }
        Ok(Some(result))
    }

    pub(super) fn finish(self) -> Result<()> {
        self.store.configuration_pending(&self.path)?;
        self.store.reconcile_configuration(&self.path)?;
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct Notifications {
    success: Option<Notice>,
    error: Option<Notice>,
    last_error: String,
}

impl Notifications {
    pub(super) fn record(&mut self, result: Notice) {
        match result {
            Ok(update) => {
                self.success = Some(Ok(update));
                self.error = None;
                self.last_error.clear();
            }
            Err(error) => {
                let message = format!("{error:#}");
                if message != self.last_error {
                    self.last_error = message;
                    self.error = Some(Err(error.context(
                        "Configuration not saved; pending library edits retained",
                    )));
                }
            }
        }
    }

    pub(super) fn deliver(&mut self, send: &mpsc::SyncSender<Notice>) -> bool {
        deliver_notice(&mut self.success, send)
            && deliver_notice(&mut self.error, send)
    }
}

fn deliver_notice(
    slot: &mut Option<Notice>,
    send: &mpsc::SyncSender<Notice>,
) -> bool {
    let Some(result) = slot.take() else {
        return true;
    };
    match send.try_send(result) {
        Ok(()) => true,
        Err(mpsc::TrySendError::Full(result)) => {
            *slot = Some(result);
            true
        }
        Err(mpsc::TrySendError::Disconnected(_)) => false,
    }
}

fn stamp(path: &Path) -> Option<(std::time::SystemTime, u64)> {
    let metadata = path.metadata().ok()?;
    Some((metadata.modified().ok()?, metadata.len()))
}

#[cfg(test)]
mod tests {
    use super::{Notifications, Update};
    use anyhow::{Result, anyhow};
    use std::sync::mpsc;

    #[test]
    fn full_notifications_keep_the_latest_complete_success() -> Result<()> {
        let (send, receive) = mpsc::sync_channel(1);
        send.send(Ok(update("already queued")))?;
        let mut pending = Notifications::default();
        pending.record(Ok(update("superseded")));
        assert!(pending.deliver(&send));
        pending.record(Ok(update("latest")));
        assert_eq!(receive.recv()??.document, "already queued");
        assert!(pending.deliver(&send));
        assert_eq!(receive.recv()??.document, "latest");
        assert!(receive.try_recv().is_err());
        Ok(())
    }

    #[test]
    fn an_error_does_not_erase_the_last_completed_configuration() -> Result<()>
    {
        let (send, receive) = mpsc::sync_channel(2);
        let mut pending = Notifications::default();
        pending.record(Ok(update("completed")));
        pending.record(Err(anyhow!("conflict")));
        assert!(pending.deliver(&send));
        assert_eq!(receive.recv()??.document, "completed");
        assert!(receive.recv()?.is_err());
        pending.record(Err(anyhow!("conflict")));
        assert!(pending.deliver(&send));
        assert!(receive.try_recv().is_err());
        Ok(())
    }

    fn update(document: &str) -> Update {
        Update {
            appearance_revision: 0,
            retention: Default::default(),
            ui: crate::config::Ui::default(),
            workspaces: Default::default(),
            catalog_fingerprint: [0; 32],
            document: document.into(),
            redacted_document: None,
            path: "configuration.toml".into(),
        }
    }
}
