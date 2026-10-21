//! SQLite owns subscriptions, cached articles and private annotations.
//!
//! Each thread opens its own connection. WAL permits reads during collection;
//! ingestion only updates publisher fields, preserving personal state. A feed's
//! validators advance in the same transaction as its articles, so a failed
//! ingest cannot make a later 304 conceal missing content. No pruning runs.

mod annotations;
mod collections;
mod comments;
mod configuration;
mod deletion;
mod documents;
mod export;
mod extraction;
mod history;
mod ingest;
mod layouts;
mod migrations;
mod notebook;
mod occurrences;
mod organization;
mod query;
mod recommendations;
mod study;
mod workspace;
pub(crate) use layouts::valid_name as valid_workspace_name;
pub(crate) use workspace::validate_dimensions as validate_workspace_dimensions;

use std::{path::Path, time::Duration};

use anyhow::{Context, Result};
use rusqlite::{Connection, Row};

use crate::model::{Article, Collection, Library};

/// A thread-owned SQLite connection. Do not share it behind an async lock.
pub struct Store {
    connection: Connection,
}

impl Store {
    pub(crate) fn write_transaction(
        &self,
    ) -> rusqlite::Result<rusqlite::Transaction<'_>> {
        // Acquire write authority before reading the state that will be changed.
        // A deferred read-to-write upgrade can otherwise fail immediately with
        // SQLITE_BUSY_SNAPSHOT while configuration publication commits in WAL.
        rusqlite::Transaction::new_unchecked(
            &self.connection,
            rusqlite::TransactionBehavior::Immediate,
        )
    }

    pub(crate) fn read_snapshot<T>(
        &self,
        read: impl FnOnce(&Self) -> Result<T>,
    ) -> Result<T> {
        if !self.connection.is_autocommit() {
            return read(self);
        }
        let transaction = self.connection.unchecked_transaction()?;
        let result = read(self)?;
        transaction.commit()?;
        Ok(result)
    }

    pub(crate) fn database_path(&self) -> Result<std::path::PathBuf> {
        self.connection
            .path()
            .filter(|path| !path.is_empty())
            .map(std::path::PathBuf::from)
            .context("Notebook workers require a file-backed library")
    }

    /// Load navigation metadata without article bodies or historical windows.
    ///
    /// # Errors
    /// Returns SQLite or invalid smart-list JSON errors.
    pub fn catalog(&self) -> Result<Library> {
        let collections = self
            .connection
            .prepare(
                "SELECT id, name FROM collections WHERE enabled = 1 ORDER BY name COLLATE NOCASE",
            )?
            .query_map([], |row| {
                Ok(Collection {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    articles: std::collections::BTreeSet::new(),
                })
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Library {
            articles: Vec::new(),
            feeds: self.feeds()?,
            collections,
            smart_lists: self.smart_lists()?,
        })
    }

    /// Open or initialize an owned database without deleting existing data.
    ///
    /// # Errors
    /// Returns filesystem, SQLite or unsupported-schema errors.
    pub fn open(path: &Path) -> Result<Self> {
        prepare_path(path)?;
        let mut connection = Connection::open(path)
            .context("Could not open the local library")?;
        connection.busy_timeout(Duration::from_millis(500))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        migrations::run(&mut connection)?;
        migrations::backfill_reading_metadata(&mut connection)?;
        Ok(Self { connection })
    }

    /// Read metadata without loading every article body into memory.
    ///
    /// # Errors
    /// Returns SQLite query or decoding errors.
    pub fn library(&self) -> Result<Library> {
        let articles = self
            .connection
            .prepare_cached(include_str!("sql/select_articles.sql"))?
            .query_map([], article_row)?
            .collect::<rusqlite::Result<_>>()?;
        Ok(Library {
            articles,
            feeds: self.feeds()?,
            collections: self.collections()?,
            smart_lists: self.smart_lists()?,
        })
    }

    /// Load one cached, terminal-safe article body.
    ///
    /// # Errors
    /// Returns an error when the article does not exist or SQLite fails.
    pub fn body(&self, id: i64) -> Result<String> {
        Ok(self.connection.query_row(
            "SELECT body FROM articles WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?)
    }

    /// Persist explicit read state.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn set_unread(&self, id: i64, unread: bool) -> Result<()> {
        self.set_flag(id, "unread", unread)
    }

    /// Persist explicit favorite state, independent of collections.
    ///
    /// # Errors
    /// Returns SQLite errors.
    pub fn set_starred(&self, id: i64, starred: bool) -> Result<()> {
        self.set_flag(id, "starred", starred)
    }

    fn collections(&self) -> Result<Vec<Collection>> {
        let mut statement = self.connection.prepare(
            "SELECT id, name FROM collections WHERE enabled = 1 ORDER BY name COLLATE NOCASE",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(Collection {
                id: row.get(0)?,
                name: row.get(1)?,
                articles: std::collections::BTreeSet::new(),
            })
        })?;
        let mut collections = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        for collection in &mut collections {
            collection.articles = self.members(collection.id)?;
        }
        Ok(collections)
    }

    fn members(&self, id: i64) -> Result<std::collections::BTreeSet<i64>> {
        Ok(self
            .connection
            .prepare_cached(
                "SELECT article_id FROM members WHERE collection_id = ?1",
            )?
            .query_map([id], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
}

#[cfg(unix)]
fn prepare_path(path: &Path) -> Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(parent)?;
    }
    // Apply private defaults only at creation, respecting existing permissions.
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path);
    match created {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            Ok(())
        }
        Err(error) => Err(error.into()),
    }
}

#[cfg(not(unix))]
fn prepare_path(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn article_row(row: &Row<'_>) -> rusqlite::Result<Article> {
    Ok(Article {
        id: row.get(0)?,
        feed_id: row.get(1)?,
        title: row.get(2)?,
        url: row.get(3)?,
        feed: row.get(4)?,
        author: row.get(5)?,
        published: row.get(6)?,
        unread: row.get(7)?,
        starred: row.get(8)?,
        comments: row.get(9)?,
        queued: row.get(10)?,
    })
}

pub(crate) use annotations::collection_name as validate_collection_name;
pub(crate) use organization::validate_tags;
