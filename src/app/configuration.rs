//! Cached configuration inspection and guarded publication of external changes.

use anyhow::Result;

use super::{Action, App, Mode};

impl App {
    /// Cache a successfully reconciled document; drawing never reads its file.
    pub fn configure_document(&mut self, update: crate::config::Update) {
        self.configuration_document = Some(update);
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
    /// Returns explicit session-flush, layout-save or metadata reload failures.
    pub fn configuration_updated(
        &mut self,
        update: crate::config::Update,
    ) -> Result<()> {
        let previous = self.configuration_document.as_ref();
        let current = crate::config::Ui::capture(&self.workspace);
        let appearance = current != update.ui
            && previous.is_none_or(|previous| previous.ui == current);
        let profiles = self.layouts.profiles != update.workspaces
            && previous.is_none_or(|previous| {
                previous.workspaces == self.layouts.profiles
            });
        let catalog = previous.is_none_or(|previous| {
            previous.catalog_fingerprint != update.catalog_fingerprint
        });
        if appearance || profiles {
            self.flush_session()?;
            if appearance {
                update.ui.apply(&mut self.workspace);
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
                let Some(config) = &self.configuration_document else {
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
