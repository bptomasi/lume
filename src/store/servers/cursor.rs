//! A downloaded payload may commit only against its captured cursor and connection.
use crate::model::{Feed, RemoteFeedUpdate};
use anyhow::Result;
use rusqlite::Connection;

pub(super) fn current(connection: &Connection, feed: &Feed) -> Result<bool> {
    let Some(server) = &feed.server else {
        return Ok(true);
    };
    if !server.enabled || !server.present {
        return Ok(false);
    }
    let cursor = serde_json::to_string(&serde_json::json!({
        "initialized": server.initialized, "changed": server.last_changed,
        "after": server.sync_after_id, "latest": server.sync_latest,
        "before": server.history.before_id, "loaded": server.history.loaded,
        "total": server.history.total,
    }))?;
    let parameters = rusqlite::named_params! {
        ":feed": feed.id, ":server": server.server_id,
        ":remote": server.remote_id, ":root": server.server_url,
        ":environment": server.token_env,
        ":token": server.token.as_ref().map_or("", |token| token.expose()),
        ":settings": serde_json::to_string(&server.sync)?, ":cursor": cursor,
    };
    Ok(connection
        .prepare_cached(include_str!(
            "../sql/servers/select_collected_context.sql"
        ))?
        .query_row(parameters, |row| row.get(0))?)
}

pub(super) fn update(
    connection: &Connection,
    remote: &RemoteFeedUpdate,
) -> Result<()> {
    connection
        .prepare_cached(include_str!("../sql/servers/update_cursor.sql"))?
        .execute((
            remote.server_id,
            remote.remote_id,
            remote.last_changed,
            remote.sync_after_id,
            remote.sync_latest,
            remote.initialized,
            remote.history.before_id,
            remote.history.loaded,
            remote.history.total,
        ))?;
    Ok(())
}

pub(super) fn server_current(
    connection: &Connection,
    server: &crate::model::Server,
) -> Result<bool> {
    if !server.enabled {
        return Ok(false);
    }
    let parameters = rusqlite::named_params! {
        ":server": server.id,
        ":root": server.url,
        ":environment": server.token_env,
        ":token": server.token.as_ref().map_or("", |token| token.expose()),
        ":settings": serde_json::to_string(&server.sync)?,
    };
    Ok(connection
        .prepare_cached(include_str!(
            "../sql/servers/select_collected_server.sql"
        ))?
        .query_row(parameters, |row| row.get(0))?)
}
