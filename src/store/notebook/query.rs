//! Bounded note pages read one snapshot and bind every user query value.

use anyhow::{Context, Result};
use rusqlite::{Row, params_from_iter, types::Value};

use crate::{
    model::{NoteKind, NoteMetadata, NotePage, NoteSummary},
    store::Store,
};

impl Store {
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
            include_str!("../sql/notebook/template_linked_notes.sql"),
            include_str!("../sql/notebook/select_note_page.sql"),
            owner = owner,
            related = related,
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
        self.note_page(text, article, Offset::Exact(offset), limit)
            .map(|(page, _)| page)
    }

    /// Search once, moving an out-of-range request to the last nonempty page.
    ///
    /// The returned offset, count and previews share one SQLite snapshot.
    /// An empty result resolves to offset zero; exact-offset callers retain
    /// `search_notes` semantics.
    ///
    /// # Errors
    /// Returns invalid expressions, stored metadata or SQLite errors.
    pub fn search_notes_clamped(
        &self,
        text: &str,
        article: Option<i64>,
        offset: usize,
        limit: usize,
    ) -> Result<(NotePage, usize)> {
        self.note_page(text, article, Offset::Clamped(offset), limit)
    }

    fn note_page(
        &self,
        text: &str,
        article: Option<i64>,
        offset: Offset,
        limit: usize,
    ) -> Result<(NotePage, usize)> {
        let (matching, mut values) = matching_notes(text, article)?;
        let transaction = self.connection.unchecked_transaction()?;
        let total = transaction.query_row(
            &format!(
                include_str!("../sql/notebook/template_count_matching.sql"),
                matching = matching,
            ),
            params_from_iter(&values),
            |row| row.get(0),
        )?;
        let limit = limit.clamp(1, 100);
        let offset = offset.resolve(total, limit);
        values.push(i64::try_from(limit)?.into());
        values.push(i64::try_from(offset)?.into());
        let sql = format!(
            include_str!("../sql/notebook/template_search_page.sql"),
            values.len() - 1,
            values.len(),
            matching = matching,
        );
        let mut notes: Vec<NoteSummary> = transaction
            .prepare_cached(&sql)?
            .query_map(params_from_iter(&values), summary_row)?
            .collect::<rusqlite::Result<_>>()?;
        super::snippets::apply(&transaction, text, &mut notes)?;
        transaction.commit()?;
        Ok((NotePage { total, notes }, offset))
    }

    /// Load one active note's preview without changing the selected article.
    ///
    /// # Errors
    /// Returns a missing/trashed note or SQLite error.
    pub fn note_summary(&self, note: i64) -> Result<NoteSummary> {
        let sql = format!(
            include_str!("../sql/notebook/template_note_summary.sql"),
            include_str!("../sql/notebook/select_note_page.sql")
        );
        self.connection
            .query_row(&sql, [note], summary_row)
            .context("Note is missing or its article is in Trash")
    }
}

enum Offset {
    Exact(usize),
    Clamped(usize),
}

impl Offset {
    fn resolve(self, total: usize, limit: usize) -> usize {
        match self {
            Self::Exact(offset) => offset,
            Self::Clamped(_) if total == 0 => 0,
            Self::Clamped(offset) if offset >= total => {
                (total - 1) / limit * limit
            }
            Self::Clamped(offset) => offset,
        }
    }
}

/// Compile the matching relation and ordered parameters before a snapshot.
fn matching_notes(
    text: &str,
    article: Option<i64>,
) -> Result<(String, Vec<Value>)> {
    let expression = crate::search::notes::compile(text)?;
    let mut values = expression.values;
    let mut predicate = format!(
        include_str!("../sql/notebook/template_search_predicate.sql"),
        expression.sql
    );
    if let Some(article) = article {
        values.push(article.into());
        predicate.push_str(&format!(
            include_str!("../sql/notebook/template_article_filter.sql"),
            values.len()
        ));
    }
    let base = include_str!("../sql/notebook/select_note_page.sql");
    let matching = format!(
        include_str!("../sql/notebook/template_matching.sql"),
        base = base,
        predicate = predicate,
    );
    Ok((matching, values))
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
