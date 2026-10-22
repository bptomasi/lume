//! Explicit web saves use a noncollecting archive, never an implicit RSS source.

use anyhow::{Result, ensure};
use rusqlite::OptionalExtension;

use crate::model::WebDocument;

use super::Store;

impl Store {
    /// Resolve readable existing content for a linked address without HTTP.
    ///
    /// # Errors
    /// Returns invalid addresses or storage errors.
    pub fn cached_page_body(&self, address: &str) -> Result<Option<String>> {
        crate::source::link_url(address)?;
        let bodies: Option<(String, Option<String>)> = self
            .connection
            .query_row(
                include_str!("sql/select_cached_web_page.sql"),
                [address],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        Ok(bodies.and_then(|(feed, full)| {
            full.into_iter().chain([feed]).find(|body| {
                crate::content::body_kind(body)
                    == crate::content::BodyKind::Text
                    && crate::source::article::rules::rejection(body).is_none()
            })
        }))
    }

    /// Save a captured page atomically; existing article identities remain intact.
    ///
    /// # Errors
    /// Returns validation/storage errors. Deleted existing pages require restore.
    pub fn save_web_document(&self, document: &WebDocument) -> Result<i64> {
        super::documents::validate_document(
            &crate::model::ExtractedDocument {
                final_url: document.url.clone(),
                body: document.body.clone(),
            },
        )?;
        ensure!(!document.title.trim().is_empty(), "Page title is empty");
        let transaction = self.write_transaction()?;
        let existing: Option<(i64, bool, bool)> = transaction
            .query_row(
                include_str!("sql/select_existing_web_page.sql"),
                [&document.url],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let article = if let Some((id, deleted, manual)) = existing {
            ensure!(!deleted, "Restore this page from Trash before saving");
            retain_linked_content(&transaction, id, manual, document)?;
            id
        } else {
            insert_page(&transaction, document)?
        };
        save_membership(&transaction, article)?;
        transaction.commit()?;
        Ok(article)
    }
}

fn retain_linked_content(
    transaction: &rusqlite::Transaction<'_>,
    article: i64,
    manual: bool,
    document: &WebDocument,
) -> Result<()> {
    let retained: bool = transaction.query_row(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM article_documents
            WHERE article_id = ?1
                AND url = ?2
                AND body != ''
        )
        "#,
        (article, &document.url),
        |row| row.get(0),
    )?;
    if !manual && !retained {
        super::documents::write_document(
            transaction,
            &crate::model::ArticleTarget {
                article,
                url: document.url.clone(),
            },
            &crate::model::ExtractedDocument {
                final_url: document.url.clone(),
                body: document.body.clone(),
            },
        )?;
    }
    Ok(())
}

fn insert_page(
    transaction: &rusqlite::Transaction<'_>,
    document: &WebDocument,
) -> Result<i64> {
    transaction.execute_batch(include_str!("sql/insert_web_archive.sql"))?;
    let feed: i64 = transaction.query_row(
        "SELECT feed_id FROM manual_sources ORDER BY feed_id LIMIT 1",
        [],
        |row| row.get(0),
    )?;
    transaction.execute(
        include_str!("sql/insert_article.sql"),
        rusqlite::params![
            feed,
            document.url,
            crate::content::label(&document.title, 200),
            document.url,
            "",
            chrono::Utc::now().timestamp(),
            document.body,
            "feed",
            crate::content::reading_minutes(&document.body),
            crate::source::canonical_article_url(&document.url),
        ],
    )?;
    Ok(transaction.last_insert_rowid())
}

fn save_membership(
    transaction: &rusqlite::Transaction<'_>,
    article: i64,
) -> Result<()> {
    transaction
        .execute(include_str!("sql/insert_saved_pages_folder.sql"), [])?;
    let folder: i64 = transaction.query_row(
        "SELECT id FROM collections WHERE name = 'Saved pages'",
        [],
        |row| row.get(0),
    )?;
    transaction.execute(
        include_str!("sql/insert_saved_page_member.sql"),
        (folder, article),
    )?;
    Ok(())
}
