//! One captured flag batch validates every target before a single SQL update.
use super::{BTreeSet, Result, Transaction, Undo, ensure};

struct State {
    article: i64,
    deleted: bool,
    unread: bool,
    starred: bool,
    queued: bool,
}

impl State {
    fn read(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            article: row.get(0)?,
            deleted: row.get(1)?,
            unread: row.get(2)?,
            starred: row.get(3)?,
            queued: row.get(4)?,
        })
    }

    fn inverse(self, column: &str, value: bool) -> Option<Undo> {
        let current = match column {
            "unread" => self.unread,
            "starred" => self.starred,
            _ => self.queued,
        };
        (current != value).then_some(Undo::Flags {
            article: self.article,
            unread: self.unread,
            starred: self.starred,
            queued: self.queued,
        })
    }
}

pub(super) fn apply(
    transaction: &Transaction<'_>,
    articles: &BTreeSet<i64>,
    column: &str,
    value: bool,
) -> Result<Vec<Undo>> {
    let ids = serde_json::to_string(articles)?;
    let rows = transaction
        .prepare_cached(include_str!("../sql/select_bulk_flags.sql"))?
        .query_map([&ids], State::read)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    ensure!(
        rows.len() == articles.len(),
        "An article no longer exists; batch cancelled"
    );
    ensure!(
        rows.iter().all(|row| !row.deleted),
        "An article moved to Trash; batch cancelled"
    );
    let previous = rows
        .into_iter()
        .filter_map(|row| row.inverse(column, value))
        .collect::<Vec<_>>();
    if !previous.is_empty() {
        // Only the three owner-selected flag names can reach this template.
        let sql = include_str!("../sql/update_bulk_flag.sql")
            .replace("{column}", column);
        transaction.prepare_cached(&sql)?.execute((value, ids))?;
    }
    Ok(previous)
}
