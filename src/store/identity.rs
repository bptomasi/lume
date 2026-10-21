//! Identify the database before changing journal mode, schema or file metadata.
//!
//! Libraries predating the application identifier must match the original
//! relational contract. An unversioned nonempty SQLite file is never adopted.

use anyhow::{Result, ensure};
use rusqlite::Connection;

pub(super) const APPLICATION_ID: i64 = 0x4c55_4d45;

pub(super) fn verify(connection: &Connection) -> Result<()> {
    let application: i64 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    ensure!(
        application == 0 || application == APPLICATION_ID,
        "This SQLite database belongs to another application"
    );
    let version: i64 =
        connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    ensure!(
        (0..=super::migrations::LATEST_SCHEMA_VERSION).contains(&version),
        "Unsupported library schema {version}; use a newer Lume"
    );
    let tables: i64 = connection.query_row(
        "SELECT count(*) FROM sqlite_schema WHERE name NOT LIKE 'sqlite_%'",
        [],
        |row| row.get(0),
    )?;
    if tables == 0 && version == 0 && application == 0 {
        return Ok(());
    }
    ensure!(
        version > 0,
        "Refusing to adopt a nonempty unversioned database"
    );
    connection
        .prepare(include_str!("sql/select_library_identity.sql"))
        .map_err(|error| {
            anyhow::anyhow!(
                "This database does not match a Lume library: {error}"
            )
        })?;
    Ok(())
}

pub(super) fn claim(connection: &Connection) -> Result<()> {
    let application: i64 =
        connection.query_row("PRAGMA application_id", [], |row| row.get(0))?;
    if application == 0 {
        connection.pragma_update(None, "application_id", APPLICATION_ID)?;
    }
    Ok(())
}
