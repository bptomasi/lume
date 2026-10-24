//! Remove only undo entries that could restore purged data or dangling edges.

use std::collections::BTreeSet;

use anyhow::Result;
use rusqlite::Transaction;

use super::Undo;
use crate::model::TagTarget;

pub(in crate::store) fn forget_articles(
    transaction: &Transaction<'_>,
    articles: &BTreeSet<i64>,
    notes: &BTreeSet<i64>,
) -> Result<()> {
    // The durable journal is bounded to 256 entries. Inspect it once per batch,
    // retaining unrelated edits and whole-batch undo semantics.
    let mut statement = transaction
        .prepare_cached(include_str!("../sql/deletion/select_history.sql"))?;
    let entries = statement
        .query_map([], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut delete = transaction
        .prepare_cached(include_str!("../sql/deletion/delete_history.sql"))?;
    for (id, payload) in entries {
        let undo: Undo = serde_json::from_str(&payload)?;
        if references(&undo, articles, notes) {
            delete.execute([id])?;
        }
    }
    Ok(())
}

fn references(
    undo: &Undo,
    articles: &BTreeSet<i64>,
    notes: &BTreeSet<i64>,
) -> bool {
    match undo {
        Undo::Batch(edits) => {
            edits.iter().any(|edit| references(edit, articles, notes))
        }
        Undo::Annotation(snapshot) => {
            articles.contains(&snapshot.article)
                || notes.contains(&snapshot.comment.id)
                || snapshot.links.iter().any(|id| articles.contains(id))
                || snapshot.metadata.references.iter().any(|(from, to)| {
                    notes.contains(from) || notes.contains(to)
                })
        }
        Undo::Tags { target, .. } => match target {
            TagTarget::Article(id) => articles.contains(id),
            TagTarget::Comment(id) => notes.contains(id),
        },
        Undo::Deletion { article, .. }
        | Undo::Flags { article, .. }
        | Undo::Comment { article, .. }
        | Undo::DeleteComment { article, .. }
        | Undo::Memberships { article, .. } => articles.contains(article),
        Undo::Collection(collection) => {
            collection.articles.iter().any(|id| articles.contains(id))
        }
        Undo::DeleteCollection(_) => false,
    }
}
