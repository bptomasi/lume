//! Recheck authority before each bounded, separately committed action batch.

use super::{Plan, protected_query};
use crate::{retention::Report, store::Store};
use anyhow::Result;
use rusqlite::Transaction;

#[derive(Clone, Copy)]
pub(crate) enum Step {
    DeleteArticles,
    ClearFull,
}

struct Entry {
    id: i64,
    feed_body: String,
    full_bytes: u64,
    feed_bytes: u64,
}

impl Store {
    pub(crate) fn retention_batch(
        &self,
        plan: &Plan,
        action: Step,
        after: i64,
        limit: usize,
        now: i64,
    ) -> Result<Option<(Report, i64)>> {
        let transaction = self.write_transaction()?;
        if !self.retention_current(plan)? {
            return Ok(None);
        }
        let sql =
            protected_query(include_str!("../sql/retention/select_batch.sql"));
        let deleting = matches!(action, Step::DeleteArticles);
        let entries = transaction
            .prepare_cached(&sql)?
            .query_map((after, plan.upper, deleting, limit), |row| {
                Ok(Entry {
                    id: row.get(0)?,
                    feed_body: row.get(1)?,
                    full_bytes: row.get(2)?,
                    feed_bytes: row.get(3)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut report = Report::default();
        for entry in &entries {
            report.add(&apply_entry(&transaction, action, entry, now)?);
        }
        transaction.commit()?;
        Ok(Some((
            report,
            entries.last().map_or(after, |entry| entry.id),
        )))
    }
}

fn apply_entry(
    transaction: &Transaction<'_>,
    action: Step,
    entry: &Entry,
    now: i64,
) -> Result<Report> {
    let mut report = Report {
        full_bytes: entry.full_bytes,
        ..Default::default()
    };
    match action {
        Step::DeleteArticles => {
            transaction
                .prepare_cached(include_str!(
                    "../sql/retention/insert_tombstones.sql"
                ))?
                .execute((entry.id, now))?;
            transaction
                .prepare_cached(include_str!(
                    "../sql/retention/delete_article.sql"
                ))?
                .execute([entry.id])?;
            report.articles = 1;
            report.feed_bytes = entry.feed_bytes;
        }
        Step::ClearFull => {
            transaction
                .prepare_cached(include_str!(
                    "../sql/update_disposable_document.sql"
                ))?
                .execute([entry.id])?;
            transaction
                .prepare_cached(include_str!(
                    "../sql/retention/update_reading_minutes.sql"
                ))?
                .execute((
                    crate::content::reading_minutes(&entry.feed_body),
                    entry.id,
                ))?;
            report.full_texts = 1;
        }
    }
    Ok(report)
}
