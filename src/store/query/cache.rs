//! Bounded page hints are valid only within an unchanged SQLite read snapshot.
use crate::model::{ArticlePage, ArticleQuery};
use anyhow::Result;
use rusqlite::{Connection, DatabaseName, TransactionState};

#[derive(Clone)]
pub(super) struct Entry {
    key: String,
    pub(super) total: usize,
    offset: usize,
    limit: usize,
    count: usize,
    first: Option<(i64, i64)>,
    last: Option<(i64, i64)>,
}

#[derive(Default)]
pub(in crate::store) struct Cache {
    epoch: Option<(i64, u64)>,
    entries: std::collections::VecDeque<Entry>,
}

pub(super) struct Seek {
    pub(super) published: i64,
    pub(super) id: i64,
    previous: bool,
}

pub(super) fn epoch(connection: &Connection) -> Result<Option<(i64, u64)>> {
    // Acquire the read snapshot before observing its version. Never reuse or
    // publish hints derived from a transaction that may subsequently roll back.
    connection.query_row(
        include_str!("../sql/query/select_snapshot.sql"),
        [],
        |_| Ok(()),
    )?;
    if connection.transaction_state(Some(DatabaseName::Main))?
        == TransactionState::Write
    {
        return Ok(None);
    }
    let version =
        connection
            .pragma_query_value(None, "data_version", |row| row.get(0))?;
    Ok(Some((version, connection.total_changes())))
}

impl Cache {
    pub(super) fn find(
        &mut self,
        epoch: Option<(i64, u64)>,
        key: &str,
    ) -> Option<Entry> {
        if epoch.is_none() || self.epoch != epoch {
            self.entries.clear();
            self.epoch = epoch;
        }
        self.entries.iter().find(|entry| entry.key == key).cloned()
    }

    pub(super) fn remember(
        &mut self,
        epoch: Option<(i64, u64)>,
        key: String,
        query: &ArticleQuery,
        page: &ArticlePage,
    ) {
        if epoch.is_none() || key.len() > 128 * 1024 {
            return;
        }
        self.entries.retain(|entry| entry.key != key);
        self.entries.push_back(Entry {
            key,
            total: page.total,
            offset: query.offset,
            limit: query.limit.clamp(1, 500),
            count: page.articles.len(),
            first: page
                .articles
                .first()
                .map(|article| (article.published, article.id)),
            last: page
                .articles
                .last()
                .map(|article| (article.published, article.id)),
        });
        while self.entries.len() > 4 {
            self.entries.pop_front();
        }
    }
}

impl Entry {
    pub(super) fn seek(&self, query: &ArticleQuery) -> Option<Seek> {
        if self.limit != query.limit.clamp(1, 500) || self.count == 0 {
            return None;
        }
        let previous =
            query.offset.checked_add(self.limit) == Some(self.offset);
        let boundary = if previous {
            self.first
        } else if self.offset.checked_add(self.count) == Some(query.offset) {
            self.last
        } else {
            None
        };
        boundary.map(|(published, id)| Seek {
            published,
            id,
            previous,
        })
    }
}

impl Seek {
    pub(super) fn sql(&self, predicate: &str) -> String {
        let selected = if self.previous {
            include_str!("../sql/query/select_before_cursor.sql")
        } else {
            include_str!("../sql/query/select_after_cursor.sql")
        };
        format!(
            "WITH matching AS MATERIALIZED ({predicate}\n{selected})\n{}\n{}",
            include_str!("../sql/select_article_metadata.sql"),
            include_str!("../sql/select_page.sql")
        )
    }
}

#[cfg(test)]
mod tests;
