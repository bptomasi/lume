//! Complete publication observations shared by projection and presentation.

use std::path::PathBuf;

use super::Ui;

/// Complete publication snapshot; catalog fingerprints bound UI refreshes.
#[derive(Clone)]
pub struct Update {
    /// Local preference generation observed in the publication transaction.
    pub appearance_revision: u64,
    /// Complete appearance, permitting safe coalescing of notifications.
    pub ui: Ui,
    /// Complete named workspace definitions, excluding undo history.
    pub workspaces:
        std::collections::BTreeMap<String, crate::model::PanelLayout>,
    /// Content fingerprint of portable catalog definitions, excluding appearance.
    pub catalog_fingerprint: [u8; 32],
    /// Validated current file text for the discoverable configuration viewer.
    pub document: String,
    /// Prepared credential-safe view; absent when the document has no inline keys.
    pub redacted_document: Option<String>,
    /// Canonical effective configuration path.
    pub path: PathBuf,
    /// Cached scheduling preferences; article scans stay on the maintenance lane.
    pub retention: super::retention::Retention,
    /// Cached automatic-backup schedule; snapshots stay on maintenance workers.
    pub backups: super::backups::Backups,
}
