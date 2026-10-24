//! Remove only undo entries that could restore purged data or dangling edges.

use anyhow::Result;
use rusqlite::Transaction;

use super::Undo;
use crate::model::TagTarget;

enum Change {
    Keep,
    Delete,
    Replace(String),
}

enum Prune {
    Keep,
    Delete,
    Updated,
}

struct Targets {
    articles: Vec<i64>,
    notes: Vec<i64>,
}

impl Targets {
    fn article(&self, id: i64) -> bool {
        self.articles.binary_search(&id).is_ok()
    }

    fn note(&self, id: i64) -> bool {
        self.notes.binary_search(&id).is_ok()
    }
}

pub(in crate::store) fn forget_articles(
    transaction: &Transaction<'_>,
) -> Result<()> {
    // Empty history needs no Rust article/note identity buffers. Otherwise keep
    // sorted flat IDs and inspect at most one private undo payload at a time.
    let populated = transaction.query_row(
        include_str!("../sql/deletion/select_has_history.sql"),
        [],
        |row| row.get::<_, bool>(0),
    )?;
    if !populated {
        return Ok(());
    }
    let targets = Targets {
        articles: identities(
            transaction,
            include_str!("../sql/deletion/select_articles.sql"),
        )?,
        notes: identities(
            transaction,
            include_str!("../sql/deletion/select_note_ids.sql"),
        )?,
    };
    for (id, change) in changes(transaction, &targets)? {
        match change {
            Change::Delete => {
                transaction.execute(
                    include_str!("../sql/deletion/delete_history.sql"),
                    [id],
                )?;
            }
            Change::Replace(payload) => {
                transaction.execute(
                    include_str!("../sql/deletion/update_history.sql"),
                    (id, payload),
                )?;
            }
            Change::Keep => {}
        }
    }
    Ok(())
}

fn changes(
    transaction: &Transaction<'_>,
    targets: &Targets,
) -> Result<Vec<(i64, Change)>> {
    let mut statement = transaction
        .prepare_cached(include_str!("../sql/deletion/select_history.sql"))?;
    let mut rows = statement.query([])?;
    let mut changes = Vec::new();
    while let Some(row) = rows.next()? {
        let payload: String = row.get(1)?;
        let undo = serde_json::from_str(&payload)?;
        let change = change(undo, targets)?;
        if !matches!(change, Change::Keep) {
            changes.push((row.get(0)?, change));
        }
    }
    Ok(changes)
}

fn change(mut undo: Undo, targets: &Targets) -> Result<Change> {
    Ok(match prune(&mut undo, targets) {
        Prune::Keep => Change::Keep,
        Prune::Delete => Change::Delete,
        Prune::Updated => Change::Replace(serde_json::to_string(&undo)?),
    })
}

fn prune(undo: &mut Undo, targets: &Targets) -> Prune {
    let changed = match undo {
        Undo::Batch(operations) => return prune_batch(operations, targets),
        Undo::DeletionBatch { articles } => {
            let previous = articles.len();
            articles.retain(|id| !targets.article(*id));
            if articles.is_empty() {
                return Prune::Delete;
            }
            articles.len() != previous
        }
        Undo::Collection(collection) => {
            let previous = collection.articles.len();
            collection.articles.retain(|id| !targets.article(*id));
            collection.articles.len() != previous
        }
        Undo::Annotation(snapshot)
            if !targets.article(snapshot.article)
                && !targets.note(snapshot.comment.id) =>
        {
            prune_annotation(snapshot, targets)
        }
        _ => {
            return if references(undo, targets) {
                Prune::Delete
            } else {
                Prune::Keep
            };
        }
    };
    if changed {
        return Prune::Updated;
    }
    Prune::Keep
}

fn prune_batch(operations: &mut Vec<Undo>, targets: &Targets) -> Prune {
    let mut updated = false;
    operations.retain_mut(|operation| match prune(operation, targets) {
        Prune::Keep => true,
        Prune::Delete => {
            updated = true;
            false
        }
        Prune::Updated => {
            updated = true;
            true
        }
    });
    if operations.is_empty() {
        return Prune::Delete;
    }
    if updated { Prune::Updated } else { Prune::Keep }
}

fn prune_annotation(
    snapshot: &mut crate::model::AnnotationSnapshot,
    targets: &Targets,
) -> bool {
    let previous = (snapshot.links.len(), snapshot.metadata.references.len());
    snapshot.links.retain(|id| !targets.article(*id));
    snapshot
        .metadata
        .references
        .retain(|(from, to)| !targets.note(*from) && !targets.note(*to));
    previous != (snapshot.links.len(), snapshot.metadata.references.len())
}

fn identities(transaction: &Transaction<'_>, sql: &str) -> Result<Vec<i64>> {
    Ok(transaction
        .prepare_cached(sql)?
        .query_map([], |row| row.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}

fn references(undo: &Undo, targets: &Targets) -> bool {
    match undo {
        Undo::Batch(edits) => {
            edits.iter().any(|edit| references(edit, targets))
        }
        Undo::Annotation(snapshot) => {
            targets.article(snapshot.article)
                || targets.note(snapshot.comment.id)
                || snapshot.links.iter().any(|id| targets.article(*id))
                || snapshot
                    .metadata
                    .references
                    .iter()
                    .any(|(from, to)| targets.note(*from) || targets.note(*to))
        }
        Undo::Tags { target, .. } => match target {
            TagTarget::Article(id) => targets.article(*id),
            TagTarget::Comment(id) => targets.note(*id),
        },
        Undo::Deletion { article, .. }
        | Undo::Flags { article, .. }
        | Undo::Comment { article, .. }
        | Undo::DeleteComment { article, .. }
        | Undo::Memberships { article, .. } => targets.article(*article),
        Undo::DeletionBatch { articles } => {
            articles.iter().any(|id| targets.article(*id))
        }
        Undo::Collection(collection) => {
            collection.articles.iter().any(|id| targets.article(*id))
        }
        Undo::DeleteCollection(_) => false,
    }
}
