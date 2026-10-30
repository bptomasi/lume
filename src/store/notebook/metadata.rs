//! Annotation metadata shares ownership and undo with the existing note.

use anyhow::{Result, ensure};
use rusqlite::{Connection, Transaction};

use crate::{
    model::NoteMetadata,
    store::{
        Store,
        history::{self, Undo},
    },
};

pub(crate) fn read_metadata(
    connection: &Connection,
    id: i64,
) -> Result<NoteMetadata> {
    let mut metadata = connection.query_row(
        include_str!("../sql/notebook/select_note_metadata.sql"),
        [id],
        |row| {
            Ok(NoteMetadata {
                kind: crate::model::NoteKind::named(&row.get::<_, String>(0)?)
                    .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            error.into(),
                        )
                    })?,
                resolved: row.get::<_, String>(1)? == "resolved",
                pinned: row.get(2)?,
                updated_at: row.get(3)?,
                references: Vec::new(),
            })
        },
    )?;
    metadata.references = connection
        .prepare_cached(include_str!(
            "../sql/notebook/select_note_references.sql"
        ))?
        .query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?
        .collect::<rusqlite::Result<_>>()?;
    Ok(metadata)
}

pub(crate) fn restore_metadata(
    transaction: &Transaction<'_>,
    id: i64,
    metadata: &NoteMetadata,
) -> Result<()> {
    // Legacy undo snapshots predate notebook metadata. Preserve their original
    // creation order instead of assigning the default Unix epoch timestamp.
    let updated_at = if metadata.updated_at == 0 {
        transaction.query_row(
            "SELECT created FROM comments WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?
    } else {
        metadata.updated_at
    };
    transaction.execute(
        include_str!("../sql/notebook/update_note_metadata.sql"),
        (
            metadata.kind.name(),
            if metadata.resolved {
                "resolved"
            } else {
                "open"
            },
            metadata.pinned,
            updated_at,
            id,
        ),
    )?;
    super::restore_references(transaction, id, &metadata.references)
}

impl Store {
    /// Update explicit type/status/pin state without changing source text.
    ///
    /// # Errors
    /// Rejects missing/wrong owners, Trash and SQLite failures.
    pub fn set_note_metadata(
        &self,
        article: i64,
        note: i64,
        metadata: &NoteMetadata,
    ) -> Result<()> {
        self.edit_metadata(article, note, |current| {
            current.kind = metadata.kind;
            current.resolved = metadata.resolved;
            current.pinned = metadata.pinned;
        })
    }

    /// Apply a property change against the current transactional note state.
    ///
    /// # Errors
    /// Rejects missing/wrong owners, Trash or SQLite failures.
    pub fn change_note_metadata(
        &self,
        article: i64,
        note: i64,
        change: crate::model::NoteChange,
    ) -> Result<()> {
        self.edit_metadata(article, note, |metadata| match change {
            crate::model::NoteChange::Kind(kind) => metadata.kind = kind,
            crate::model::NoteChange::Resolved(resolved) => {
                metadata.resolved = resolved
            }
            crate::model::NoteChange::ToggleResolved => {
                metadata.resolved = !metadata.resolved
            }
            crate::model::NoteChange::TogglePin => {
                metadata.pinned = !metadata.pinned
            }
        })
    }

    fn edit_metadata(
        &self,
        article: i64,
        note: i64,
        edit: impl FnOnce(&mut NoteMetadata),
    ) -> Result<()> {
        let transaction = self.write_transaction()?;
        ensure!(!self.is_deleted(article)?, "Restore the article first");
        let before =
            super::super::study::snapshot(&transaction, article, note)?;
        let mut metadata = before.metadata.clone();
        edit(&mut metadata);
        transaction.execute(
            include_str!("../sql/notebook/update_note_metadata.sql"),
            (
                metadata.kind.name(),
                if metadata.resolved {
                    "resolved"
                } else {
                    "open"
                },
                metadata.pinned,
                chrono::Utc::now().timestamp(),
                note,
            ),
        )?;
        history::record(&transaction, &Undo::Annotation(Box::new(before)))?;
        transaction.commit()?;
        Ok(())
    }
}
