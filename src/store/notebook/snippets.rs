//! SQLite owns tokenization, phrase matching and bounded context snippets.

use anyhow::Result;
use rusqlite::{Transaction, params_from_iter, types::Value};

use crate::model::NoteSummary;

pub(super) fn apply(
    transaction: &Transaction<'_>,
    query: &str,
    notes: &mut [NoteSummary],
) -> Result<()> {
    let Some(query) = crate::search::notes::preview_query(query)? else {
        return Ok(());
    };
    if notes.is_empty() {
        return Ok(());
    }
    let mut values = vec![Value::Text(query)];
    values.extend(notes.iter().map(|note| Value::Integer(note.id)));
    let placeholders = (2..=values.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(", ");
    let sql = format!(
        "{} AND rowid IN ({placeholders})",
        include_str!("../sql/notebook/select_match_snippets.sql")
    );
    let mut statement = transaction.prepare_cached(&sql)?;
    let rows = statement.query_map(params_from_iter(&values), |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    for row in rows {
        let (id, preview) = row?;
        if let Some(note) = notes.iter_mut().find(|note| note.id == id) {
            note.preview = preview;
        }
    }
    Ok(())
}
