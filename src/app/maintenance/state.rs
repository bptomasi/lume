//! Admission, cancellation, and acknowledgment own the durable worker slot.

use std::{path::PathBuf, sync::atomic::Ordering, sync::mpsc};

use anyhow::{Context, Result, ensure};

use super::{Maintenance, Request, Update, diagnostics::Diagnostics};
use crate::storage::Unit;

#[derive(Default)]
pub(in crate::app) struct MaintenanceState {
    worker: Option<Maintenance>,
    diagnostics: Option<Diagnostics>,
}

impl MaintenanceState {
    pub(super) fn admit(
        &mut self,
        path: PathBuf,
        request: Request,
    ) -> Result<()> {
        if self.worker.is_none() {
            self.worker = Some(Maintenance::start(path)?);
        }
        let worker = self.worker.as_mut().context("Maintenance unavailable")?;
        ensure!(
            !worker.pending,
            "Maintenance is busy; :offline cancel requests cancellation"
        );
        let cleaning = matches!(request, Request::Clean(_));
        let cancelled = worker.cancel.swap(false, Ordering::Relaxed);
        if let Err(error) = worker.requests.try_send(request) {
            worker.cancel.store(cancelled, Ordering::Relaxed);
            return Err(error).context("Maintenance request was not admitted");
        }
        worker.pending = true;
        worker.cleaning = cleaning;
        Ok(())
    }

    pub(super) fn poll(&mut self) -> Result<Option<Update>> {
        let Some(worker) = &mut self.worker else {
            return Ok(None);
        };
        let update = match worker.results.try_recv() {
            Ok(update) => update,
            Err(mpsc::TryRecvError::Empty) => return Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => {
                let pending = worker.pending;
                worker.pending = false;
                worker.cleaning = false;
                ensure!(
                    !pending,
                    "Maintenance worker stopped before acknowledgement; inspect its checkpoint"
                );
                return Ok(None);
            }
        };
        if matches!(update, Update::Finished(_)) {
            worker.pending = false;
            worker.cleaning = false;
        }
        Ok(Some(update))
    }

    pub(super) fn durable_pending(&self) -> bool {
        self.worker.as_ref().is_some_and(|worker| worker.pending)
    }

    pub(super) fn pending(&self) -> bool {
        self.durable_pending() || self.diagnostic_pending()
    }

    fn diagnostic_pending(&self) -> bool {
        self.diagnostics.as_ref().is_some_and(Diagnostics::pending)
    }

    pub(super) fn cancel(&self) {
        if let Some(worker) = &self.worker {
            worker.cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Storage cancellation may stop cleanup, but leaves other durable work alone.
    pub(super) fn cancel_storage(&mut self) -> bool {
        if let Some(diagnostics) = &mut self.diagnostics {
            diagnostics.cancel();
        }
        let cleaning = self
            .worker
            .as_ref()
            .is_some_and(|worker| worker.pending && worker.cleaning);
        if cleaning {
            self.cancel();
        }
        cleaning
    }

    pub(super) fn inspect(
        &mut self,
        path: PathBuf,
        unit: Unit,
        details: bool,
    ) -> Result<()> {
        if self.diagnostics.is_none() {
            self.diagnostics = Some(Diagnostics::start(path)?);
        }
        if let Some(diagnostics) = &mut self.diagnostics {
            diagnostics.submit(unit, details);
        }
        Ok(())
    }

    pub(super) fn poll_diagnostic(&mut self) -> Option<Result<String>> {
        self.diagnostics.as_mut()?.poll()
    }
}

#[cfg(test)]
#[path = "diagnostics/tests.rs"]
mod tests;
