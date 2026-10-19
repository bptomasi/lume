//! Bounded note pages read one snapshot and bind every user query value.

use anyhow::{Context, Result};
use rusqlite::{Row, params_from_iter};

use crate::{
    model::{NoteKind, NoteMetadata, NotePage, NoteSummary},
    store::Store,
};

impl Store {
    /// Load the selected note, references and source in one read transaction.
    ///
    /// # Errors
    /// Returns missing/trashed notes, corrupt metadata or SQLite errors.
    pub fn note_detail(&self, id: i64) -> Result<crate::model::NoteDetail> {
        let transaction = self.connection.unchecked_transaction()?;
        let summary = self.note_summary(id)?;
        let article = self.article(summary.article)?;
        let note = self.annotation(summary.article, id)?;
        let (body, source_available, source_revision) =
            if note.comment.source == crate::model::ContentSource::Full {
                self.cached_document(summary.article)?
                    .filter(|document| {
                        document.url == article.url && !document.body.is_empty()
                    })
                    .map_or_else(
                        || (String::new(), false, 0),
                        |document| (document.body, true, document.revision),
                    )
            } else {
                (self.body(summary.article)?, true, 0)
            };
        let detail = crate::model::NoteDetail {
            backlinks: self.note_backlinks(id)?,
            references: self.note_references(id)?,
            summary,
            note,
            url: article.url,
            body,
            source_available,
            source_revision,
        };
        transaction.commit()?;
        Ok(detail)
    }

    /// Read at most 128 active notes that reference this note.
    ///
    /// # Errors
    /// Returns SQLite/metadata errors.
    pub fn note_backlinks(&self, note: i64) -> Result<Vec<NoteSummary>> {
        self.linked_notes(note, "target_id", "source_id")
    }

    /// Read at most 128 active notes referenced by this note.
    ///
    /// # Errors
    /// Returns SQLite/metadata errors.
    pub fn note_references(&self, note: i64) -> Result<Vec<NoteSummary>> {
        self.linked_notes(note, "source_id", "target_id")
    }

    fn linked_notes(
        &self,
        note: i64,
        owner: &str,
        related: &str,
    ) -> Result<Vec<NoteSummary>> {
        let sql = format!(
            r#"{}
            WHERE a.deleted = 0 AND c.id IN (
                SELECT {related} FROM note_references WHERE {owner} = ?1
            )
            ORDER BY c.id
            LIMIT 128"#,
            include_str!("../sql/notebook/select_note_page.sql"),
        );
        Ok(self
            .connection
            .prepare_cached(&sql)?
            .query_map([note], summary_row)?
            .collect::<rusqlite::Result<_>>()?)
    }

    /// Search annotations independently, returning at most 100 previews.
    ///
    /// # Errors
    /// Returns invalid expressions, stored metadata or SQLite errors.
    pub fn search_notes(
        &self,
        text: &str,
        article: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<NotePage> {
        let expression = crate::search::notes::compile(text)?;
        let mut values = expression.values;
        let mut predicate = format!("a.deleted = 0 AND ({})", expression.sql);
        if let Some(article) = article {
            values.push(article.into());
            predicate
                .push_str(&format!(" AND c.article_id = ?{}", values.len()));
        }
        let base = include_str!("../sql/notebook/select_note_page.sql");
        let matching = format!("{base}\nWHERE {predicate}");
        let transaction = self.connection.unchecked_transaction()?;
        let total = transaction.query_row(
            &format!("SELECT count(*) FROM ({matching})"),
            params_from_iter(&values),
            |row| row.get(0),
        )?;
        values.push(i64::try_from(limit.clamp(1, 100))?.into());
        values.push(i64::try_from(offset)?.into());
        let sql = format!(
            r#"{matching}
            ORDER BY m.pinned DESC, m.updated_at DESC, c.id DESC
            LIMIT ?{} OFFSET ?{}"#,
            values.len() - 1,
            values.len()
        );
        let mut notes: Vec<NoteSummary> = transaction
            .prepare_cached(&sql)?
            .query_map(params_from_iter(&values), summary_row)?
            .collect::<rusqlite::Result<_>>()?;
        super::snippets::apply(&transaction, text, &mut notes)?;
        transaction.commit()?;
        Ok(NotePage { total, notes })
    }

    /// Load one active note's preview without changing the selected article.
    ///
    /// # Errors
    /// Returns a missing/trashed note or SQLite error.
    pub fn note_summary(&self, note: i64) -> Result<NoteSummary> {
        let sql = format!(
            "{}\nWHERE c.id = ?1 AND a.deleted = 0",
            include_str!("../sql/notebook/select_note_page.sql")
        );
        self.connection
            .query_row(&sql, [note], summary_row)
            .context("Note is missing or its article is in Trash")
    }
}

fn summary_row(row: &Row<'_>) -> rusqlite::Result<NoteSummary> {
    let kind: String = row.get(7)?;
    let tags: String = row.get(11)?;
    let decode = |column, error: anyhow::Error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            error.into(),
        )
    };
    Ok(NoteSummary {
        id: row.get(0)?,
        article: row.get(1)?,
        title: row.get(2)?,
        source: row.get(3)?,
        body: row.get(4)?,
        preview: String::new(),
        quote: row.get(5)?,
        created: row.get(6)?,
        metadata: NoteMetadata {
            kind: NoteKind::named(&kind).map_err(|error| decode(7, error))?,
            resolved: row.get::<_, String>(8)? == "resolved",
            pinned: row.get(9)?,
            updated_at: row.get(10)?,
            references: Vec::new(),
        },
        tags: serde_json::from_str(&tags)
            .map_err(|error| decode(11, error.into()))?,
    })
}
