//! Writes turn new drafts into captured updates before the editor can continue.
//!
//! Store returns identities inside the creating transaction. Repeated :w never
//! finds an identity by querying the newest row or creates duplicate notes.

use anyhow::Result;

use super::{App, Input, InputKind};

impl App {
    pub(super) fn write_draft(&mut self, input: &mut Input) -> Result<()> {
        if !input.modified() && persisted(&input.kind) {
            self.status = "Buffer already saved".into();
            return Ok(());
        }
        let draft = super::recovery::snapshot(input).ok_or_else(|| {
            anyhow::anyhow!("Open a note or study-card editor first")
        })?;
        let target = self.store.write_note_draft(&draft)?;
        input.kind = InputKind::from_draft_target(target);
        input.saved_text.clone_from(&input.text);
        input.initial_kind = None;
        if let Some(target) = input.draft_target() {
            self.refresh_notes(target.article())?;
        }
        self.status =
            "Buffer saved · :wq closes · :wqa saves and quits Lume".into();
        Ok(())
    }
}

pub(super) fn persisted(kind: &InputKind) -> bool {
    matches!(
        kind,
        InputKind::Comment {
            comment: Some(_),
            ..
        } | InputKind::StudyCard { card: Some(_), .. }
    )
}
