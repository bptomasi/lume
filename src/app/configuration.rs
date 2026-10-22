//! Cached configuration inspection and guarded publication of external changes.

use anyhow::Result;

use super::{Action, App, Mode};

#[derive(Default)]
pub(super) struct ConfigurationState {
    document: Option<crate::config::Update>,
    deferred: Option<Deferred>,
    written_ui: Option<crate::config::Ui>,
}

struct Deferred {
    update: crate::config::Update,
    barrier: Option<std::sync::mpsc::Receiver<()>>,
}

impl App {
    /// Cache a successfully reconciled document; drawing never reads its file.
    pub fn configure_document(&mut self, update: crate::config::Update) {
        self.configuration.document = Some(update);
    }

    pub(super) fn remember_appearance_write(&mut self) {
        let current = crate::config::Ui::capture(&self.workspace);
        let previous = self.configuration.written_ui.as_ref().or_else(|| {
            self.configuration
                .document
                .as_ref()
                .map(|update| &update.ui)
        });
        if previous.is_some_and(|previous| *previous != current) {
            // Advance even when returning to the initial value. Delayed
            // publications carry this durable revision and cannot undo it.
            self.workspace.appearance_revision =
                self.workspace.appearance_revision.saturating_add(1);
        }
        self.configuration.written_ui = Some(current);
    }

    /// Whether external definitions can be installed without interrupting edits.
    pub fn configuration_idle(&self) -> bool {
        matches!(self.mode, Mode::Normal)
            && self.selection.is_none()
            && self.notebook.is_none()
            && self.panel_drag().is_none()
            && !self.pane_motion_active()
    }

    /// Apply external preferences only if no newer appearance edit intervened.
    ///
    /// # Errors
    /// Returns barrier-admission, layout-save or metadata reload failures.
    pub fn configuration_updated(
        &mut self,
        update: crate::config::Update,
    ) -> Result<()> {
        if self.session_pending() || !self.configuration_idle() {
            self.configuration.deferred = Some(Deferred {
                update,
                barrier: None,
            });
            return Ok(());
        }
        self.install_configuration(update)
    }

    pub(super) fn finish_configuration_preferences(&mut self) -> Result<()> {
        // Called only after the session barrier at shutdown. Local-edit guards
        // still apply, but an open note must not suppress external preferences.
        if let Some(deferred) = self.configuration.deferred.take() {
            self.install_configuration(deferred.update)?;
        }
        Ok(())
    }

    pub(super) fn poll_configuration(&mut self) -> Result<bool> {
        if !self.configuration_idle() {
            return Ok(false);
        }
        let Some(deferred) = &mut self.configuration.deferred else {
            return Ok(false);
        };
        if let Some(writer) = &self.session_writer {
            if deferred.barrier.is_none() {
                deferred.barrier = writer.barrier()?;
            }
            let Some(barrier) = &deferred.barrier else {
                return Ok(false);
            };
            match barrier.try_recv() {
                Ok(()) => {}
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(false),
                Err(error) => return Err(error.into()),
            }
            anyhow::ensure!(!writer.failed(), "Session writes failed");
        }
        if let Some(deferred) = self.configuration.deferred.take() {
            self.install_configuration(deferred.update)?;
        }
        Ok(true)
    }

    fn install_configuration(
        &mut self,
        update: crate::config::Update,
    ) -> Result<()> {
        let previous = self.configuration.document.as_ref();
        let current = crate::config::Ui::capture(&self.workspace);
        let appearance = current != update.ui
            && update.appearance_revision >= self.workspace.appearance_revision
            && previous.is_none_or(|previous| previous.ui == current);
        let profiles = self.layouts.profiles != update.workspaces
            && previous.is_none_or(|previous| {
                previous.workspaces == self.layouts.profiles
            });
        let catalog = previous.is_none_or(|previous| {
            previous.catalog_fingerprint != update.catalog_fingerprint
        });
        if appearance || profiles {
            if appearance {
                let justify = self.workspace.justify;
                update.ui.apply(&mut self.workspace);
                self.apply_image_preferences();
                self.apply_formula_preferences();
                if self.workspace.justify != justify {
                    self.cache_reader_rows();
                }
            }
            if profiles {
                self.layouts.profiles = update.workspaces.clone();
            }
            self.persist_preferences()?;
        }
        self.configure_document(update);
        if catalog {
            self.reload()?;
        }
        self.status = "Configuration updated".into();
        Ok(())
    }

    pub(super) fn configuration_command(
        &mut self,
        verb: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "config" => {
                let Some(config) = &self.configuration.document else {
                    anyhow::bail!(
                        "No configuration file in this session; demo mode is isolated"
                    );
                };
                let text = format!(
                    "# Configuration\n\n{}\n\nEdit this file externally, \
                    then use :config-reload.\n\n```toml\n{}\n```",
                    config.path.display(),
                    config.document
                );
                self.show_document("Configuration · Esc closes", &text, None);
                Ok(Some(Action::None))
            }
            "config-reload" => Ok(Some(Action::ReloadConfiguration)),
            _ => Ok(None),
        }
    }
}
