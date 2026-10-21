//! Transactional projection and durable publication base for declarative data.
//!
//! Lock order is configuration lock, then SQLite IMMEDIATE transaction. Atomic
//! file publication precedes SQLite acknowledgement. A crash leaves either the
//! old file or the complete new file; the recorded base permits replay of both.

mod apply;
mod bindings;
mod snapshot;

use std::path::Path;

use anyhow::{Result, ensure};
use sha2::Digest;

use crate::config::{self, Configuration, Update};

use super::Store;

impl Store {
    pub(crate) fn recover_configuration(
        &self,
        path: &Path,
        source: config::Recovery,
    ) -> Result<Vec<std::path::PathBuf>> {
        use std::io::Write;
        let _lock = config::publication::lock(path)?;
        let transaction = self.write_transaction()?;
        let file = config::publication::read(path)?;
        let local = self.capture_configuration()?;
        let chosen = match source {
            config::Recovery::File => {
                config::decode(file.as_deref().ok_or_else(|| {
                    anyhow::anyhow!("Configuration file does not exist")
                })?)?
            }
            config::Recovery::Library => local.clone(),
        };
        let mut backups = Vec::new();
        for (label, text) in [
            ("file", file.unwrap_or_default()),
            ("library", config::encode(&local)?),
        ] {
            let mut backup = tempfile::Builder::new()
                .prefix(".lume-config-recovery-")
                .suffix(&format!("-{label}.toml"))
                .tempfile_in(path.parent().unwrap_or(Path::new(".")))?;
            backup.write_all(text.as_bytes())?;
            backup.as_file().sync_all()?;
            backups.push(backup.keep()?.1);
        }
        self.apply_configuration(&chosen, &local)?;
        let text = config::encode(&chosen)?;
        let previous = config::publication::read(path)?;
        config::publication::publish(path, previous.as_deref(), &text)?;
        self.connection.execute(
            include_str!("sql/configuration/update_publication.sql"),
            (path.to_str(), serde_json::to_string(&chosen)?),
        )?;
        transaction.commit()?;
        Ok(backups)
    }

    pub(crate) fn configuration_pending(&self, path: &Path) -> Result<bool> {
        let state = self.configuration_state()?;
        ensure!(
            Some(state.path.as_str()) == path.to_str(),
            "Library configuration path changed; reopen Lume using {}",
            state.path
        );
        Ok(state.pending)
    }

    pub(crate) fn reconcile_configuration(
        &self,
        path: &Path,
    ) -> Result<Update> {
        let _lock = config::publication::lock(path)?;
        let transaction = self.write_transaction()?;
        let text = config::publication::read(path)?;
        let base = self.configuration_base(path)?;
        let file = text.as_deref().map(config::decode).transpose()?;
        if let Some(file) = &file
            && base.as_ref() != Some(file)
        {
            let adoption = if base.is_none() {
                bindings::Adoption::Initial
            } else {
                bindings::Adoption::Unbound
            };
            self.adopt_configuration_keys(file, adoption)?;
        }
        let current = self.capture_configuration()?;
        let merged = reconcile(base.as_ref(), &current, file.as_ref())?;
        self.apply_configuration(&merged, &current)?;
        let published = match &text {
            Some(text) if file.as_ref() == Some(&merged) => text.clone(),
            Some(text) => config::document::patch(text, &merged)?,
            None => config::document::patch(
                "# Lume configuration. Personal reading data stays in SQLite.\nschema_version = 1\n",
                &merged,
            )?,
        };
        config::publication::publish(path, text.as_deref(), &published)?;
        self.connection.execute(
            include_str!("sql/configuration/update_publication.sql"),
            (path.to_str(), serde_json::to_string(&merged)?),
        )?;
        let update = publication_update(path, merged, published)?;
        transaction.commit()?;
        Ok(update)
    }

    fn configuration_state(&self) -> Result<PublicationState> {
        Ok(self.connection.query_row(
            include_str!("sql/configuration/select_publication.sql"),
            [],
            |row| {
                Ok(PublicationState {
                    path: row.get(0)?,
                    snapshot: row.get(1)?,
                    pending: row.get(2)?,
                })
            },
        )?)
    }

    fn configuration_base(&self, path: &Path) -> Result<Option<Configuration>> {
        let state = self.configuration_state()?;
        let path = path.to_str().ok_or_else(|| {
            anyhow::anyhow!("Configuration path must be UTF-8")
        })?;
        if state.path != path {
            ensure!(
                !state.pending || state.path.is_empty(),
                "Finish pending configuration edits in {} before selecting another file",
                state.path
            );
            return Ok(None);
        }
        if state.snapshot.is_empty() {
            return Ok(None);
        }
        Ok(Some(serde_json::from_str(&state.snapshot)?))
    }
}

struct PublicationState {
    path: String,
    snapshot: String,
    pending: bool,
}

fn publication_update(
    path: &Path,
    config: Configuration,
    document: String,
) -> Result<Update> {
    let catalog = serde_json::to_vec(&(
        &config.feeds,
        &config.folders,
        &config.smart_lists,
        &config.note_collections,
        &config.recommendations,
    ))?;
    Ok(Update {
        ui: config.ui,
        workspaces: config.workspaces,
        catalog_fingerprint: sha2::Sha256::digest(catalog).into(),
        document,
        path: path.to_path_buf(),
    })
}

fn reconcile(
    base: Option<&Configuration>,
    current: &Configuration,
    file: Option<&Configuration>,
) -> Result<Configuration> {
    match (base, file) {
        (Some(base), Some(file)) => {
            config::merge::configuration(base, current, file)
        }
        (None, Some(file)) => config::merge::configuration(
            &Configuration::default(),
            current,
            file,
        ),
        (None, None) => Ok(current.clone()),
        (Some(_), None) => anyhow::bail!(
            "Configuration file disappeared; pending \
            edits retained. Restore it or use `lume config recover library`"
        ),
    }
}
