//! Replaceable storage reads never queue behind durable maintenance.
use super::App;
use crate::{disk_usage::Unit, latest::Latest, store::Store};
use anyhow::{Context, Result};

const TITLE: &str = "Storage inspection · :storage cancel · Esc closes";

pub(in crate::app) struct Diagnostics {
    worker: Latest<(Unit, bool), String>,
}

impl Diagnostics {
    pub(super) fn start(path: std::path::PathBuf) -> Result<Self> {
        let images = path.parent().context("Library directory")?.join("images");
        let mut store = None;
        let worker = Latest::start_cancellable(
            "lume-storage-inspection",
            move |(unit, details), cancel| {
                cancel.check()?;
                if store.is_none() {
                    store = Some(Store::open(&path)?);
                }
                let store = store.as_mut().context("Inspection connection")?;
                store.read_cancellable(cancel.clone(), |store| {
                    if details {
                        let details =
                            crate::disk_usage::details::inspect(store)?;
                        cancel.check()?;
                        Ok(details.text(unit))
                    } else {
                        crate::disk_usage::inspect_checked(
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

    pub(super) fn submit(&mut self, unit: Unit, details: bool) {
        self.worker.submit((unit, details));
    }

    pub(super) fn poll(&mut self) -> Option<Result<String>> {
        self.worker.poll()
    }

    pub(super) fn cancel(&mut self) {
        self.worker.cancel();
    }
}

impl App {
    pub(super) fn submit_storage_diagnostic(
        &mut self,
        unit: Unit,
        details: bool,
    ) -> Result<()> {
        self.maintenance
            .inspect(self.store.database_path()?, unit, details)?;
        self.show_document(
            TITLE,
            "Inspecting storage in the background…",
            None,
        );
        Ok(())
    }

    pub(super) fn poll_storage_diagnostic(&mut self) -> Result<bool> {
        let Some(result) = self.maintenance.poll_diagnostic() else {
            return Ok(false);
        };
        let visible = matches!(self.interaction.mode(), crate::app::Mode::Document(view) if view.title == TITLE);
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
        let cleaning = self.maintenance.cancel_storage();
        if matches!(self.interaction.mode(), crate::app::Mode::Document(view) if view.title == TITLE)
        {
            self.show_document(
                "Storage inspection cancelled · Esc closes",
                "Inspection cancelled.",
                None,
            );
        }
        self.interaction.report(
            if cleaning {
                "Storage cancellation requested"
            } else {
                "Storage inspection cancelled"
            }
            .into(),
        );
    }
}
