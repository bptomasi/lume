//! Replaceable storage reads never queue behind durable maintenance.
use super::App;
use crate::{latest::Latest, storage::Unit, store::Store};
use anyhow::{Context, Result};

const TITLE: &str = "Storage inspection · :storage cancel · Esc closes";

pub(in crate::app) struct Diagnostics {
    worker: Latest<(Unit, bool), String>,
}

impl Diagnostics {
    fn start(path: std::path::PathBuf) -> Result<Self> {
        let images = path.parent().context("Library directory")?.join("images");
        let mut store = None;
        let worker = Latest::start_cancellable(
            "lume-storage-inspection",
            move |(unit, details), cancel| {
                cancel.check()?;
                if store.is_none() {
                    store = Some(Store::open(&path)?);
                }
                let store = store.as_ref().context("Inspection connection")?;
                store.read_cancellable(cancel.clone(), |store| {
                    if details {
                        let details = crate::storage::details::inspect(store)?;
                        cancel.check()?;
                        Ok(details.text(unit))
                    } else {
                        crate::storage::inspect_checked(
                            store,
                            &images,
                            unit,
                            &|| cancel.check(),
                        )
                    }
                })
            },
        )?;
        Ok(Self { worker })
    }

    pub(super) fn pending(&self) -> bool {
        self.worker.pending()
    }
}

impl App {
    pub(super) fn submit_storage_diagnostic(
        &mut self,
        unit: Unit,
        details: bool,
    ) -> Result<()> {
        if self.storage_diagnostics.is_none() {
            self.storage_diagnostics =
                Some(Diagnostics::start(self.store.database_path()?)?);
        }
        if let Some(diagnostics) = &mut self.storage_diagnostics {
            diagnostics.worker.submit((unit, details));
        }
        self.show_document(
            TITLE,
            "Inspecting storage in the background…",
            None,
        );
        Ok(())
    }

    pub(super) fn poll_storage_diagnostic(&mut self) -> Result<bool> {
        let result = self
            .storage_diagnostics
            .as_mut()
            .and_then(|diagnostics| diagnostics.worker.poll());
        let Some(result) = result else {
            return Ok(false);
        };
        let visible = matches!(&self.mode, crate::app::Mode::Document(view) if view.title == TITLE);
        match result {
            Ok(text) if visible => {
                self.show_document("Storage · Esc closes", &text, None)
            }
            Err(error) => {
                if visible {
                    self.show_document(
                        "Storage inspection failed · Esc closes",
                        &format!("{error:#}"),
                        None,
                    );
                }
                return Err(error);
            }
            Ok(_) => {}
        }
        Ok(true)
    }

    pub(in crate::app) fn cancel_storage_diagnostic(&mut self) {
        if let Some(diagnostics) = &mut self.storage_diagnostics {
            diagnostics.worker.cancel();
        }
        let cleaning = self
            .maintenance
            .as_ref()
            .is_some_and(|worker| worker.pending && worker.cleaning);
        if cleaning && let Some(worker) = &self.maintenance {
            worker
                .cancel
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        if matches!(&self.mode, crate::app::Mode::Document(view) if view.title == TITLE)
        {
            self.show_document(
                "Storage inspection cancelled · Esc closes",
                "Inspection cancelled.",
                None,
            );
        }
        self.status = if cleaning {
            "Storage cancellation requested"
        } else {
            "Storage inspection cancelled"
        }
        .into();
    }
}

#[cfg(test)]
mod tests;
