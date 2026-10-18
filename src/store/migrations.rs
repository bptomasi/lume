//! Adopt an existing version-one library without rewriting its bootstrap.
//!
//! The original bootstrap remains immutable. Refinery records and verifies
//! every subsequent SQL migration. Grouped execution keeps schema changes and
//! migration history atomic, including FTS indexing of existing annotations.

use anyhow::{Result, ensure};
use refinery::{Migration, Runner};
use rusqlite::{Connection, OptionalExtension, Transaction};

pub(super) fn run(connection: &mut Connection) -> Result<()> {
    let version: i64 =
        connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    ensure!(
        version <= 8,
        "Unsupported library schema {version}; use a newer Lume"
    );
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(include_str!("sql/create_schema.sql"))?;
        transaction.commit()?;
    }
    let migrations = MIGRATIONS
        .iter()
        .map(|(name, sql)| Migration::unapplied(name, sql))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Runner::new(&migrations).set_grouped(true).run(connection)?;
    Ok(())
}

pub(super) fn backfill_reading_metadata(
    connection: &mut Connection,
) -> Result<()> {
    let done: Option<i64> = connection
        .query_row(
            "SELECT 1 FROM settings WHERE key = 'reading_metadata_v7'",
            [],
            |row| row.get(0),
        )
        .optional()?;
    if done.is_some() {
        return Ok(());
    }
    let transaction = connection.transaction()?;
    let mut cursor = 0_i64;
    while let Some(next) = backfill_page(&transaction, cursor)? {
        cursor = next;
    }
    transaction.execute(
        "INSERT INTO settings (key, value) VALUES ('reading_metadata_v7', '1')",
        [],
    )?;
    transaction.commit()?;
    Ok(())
}

fn backfill_page(
    transaction: &Transaction<'_>,
    cursor: i64,
) -> Result<Option<i64>> {
    let rows = transaction
        .prepare_cached(include_str!("sql/select_reading_backfill.sql"))?
        .query_map([cursor], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut next = None;
    for (id, body, url) in rows {
        transaction.execute(
            r#"
            UPDATE articles
            SET reading_minutes = ?1,
                canonical_key = ?2
            WHERE id = ?3
            "#,
            (
                crate::content::reading_minutes(&body) as i64,
                crate::source::canonical_article_url(&url),
                id,
            ),
        )?;
        next = Some(id);
    }
    Ok(next)
}

const MIGRATIONS: &[(&str, &str)] = &[
    (
        "V1__index_owned_library",
        include_str!("sql/migrations/V1__index_owned_library.sql"),
    ),
    (
        "V2__extend_reading_workspace",
        include_str!("sql/migrations/V2__extend_reading_workspace.sql"),
    ),
    (
        "V3__add_article_trash",
        include_str!("sql/migrations/V3__add_article_trash.sql"),
    ),
    (
        "V4__cache_full_articles",
        include_str!("sql/migrations/V4__cache_full_articles.sql"),
    ),
    (
        "V5__configure_extraction_health",
        include_str!("sql/migrations/V5__configure_extraction_health.sql"),
    ),
    (
        "V6__organize_study_library",
        include_str!("sql/migrations/V6__organize_study_library.sql"),
    ),
    (
        "V7__group_article_occurrences",
        include_str!("sql/migrations/V7__group_article_occurrences.sql"),
    ),
    (
        "V8__index_read_visibility",
        include_str!("sql/migrations/V8__index_read_visibility.sql"),
    ),
];

#[cfg(test)]
mod tests;
