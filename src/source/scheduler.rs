//! Four bounded download lanes, with one lane per domain and one SQLite writer.
//!
//! Domain groups never overlap across lanes. Ordinary feed starts are 500 ms apart;
//! private reader API calls are sequential without artificial spacing. Each lane
//! carries Retry-After forward to its remaining feeds. Results cross a
//! four-slot channel; slow persistence cannot accumulate unbounded documents.

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

use anyhow::Result;

use crate::{model::Feed, store::Store};

use super::{
    Progress,
    download::{self, Download},
};

pub(super) fn refresh(
    path: &Path,
    client: &ureq::Agent,
    stopped: &AtomicBool,
    progress: impl Fn(usize, usize),
) -> Result<Progress> {
    let mut store = Store::open(path)?;
    discovery::collect(
        &mut store,
        stopped,
        progress,
        &|server| super::miniflux::discover(client, server, stopped),
        &|feed| download::fetch(client, feed, stopped),
    )
}

#[cfg(test)]
fn collect(
    store: &mut Store,
    stopped: &AtomicBool,
    progress: impl Fn(usize, usize),
    fetch: &(impl Fn(&Feed) -> Download + Sync),
) -> Result<Progress> {
    let mut candidates: BTreeMap<i64, Vec<crate::model::RemoteFeedCandidate>> =
        BTreeMap::new();
    for feed in store.feeds()? {
        if let Some(server) = feed.server {
            candidates.entry(server.server_id).or_default().push(
                crate::model::RemoteFeedCandidate {
                    remote_id: server.remote_id,
                    title: feed.title,
                    remote_url: feed.url.clone(),
                    local_url: feed.url,
                },
            );
        }
    }
    discovery::collect(
        store,
        stopped,
        progress,
        &|server| Ok(candidates.get(&server.id).cloned().unwrap_or_default()),
        fetch,
    )
}

#[cfg(test)]
fn eligible_feeds(store: &Store) -> Result<Vec<Feed>> {
    let now = chrono::Utc::now().timestamp();
    let retries = store.domain_retries(now)?;
    let health = store
        .source_health()?
        .into_iter()
        .map(|source| (source.feed, source.retry_at))
        .collect::<BTreeMap<_, _>>();
    let failed_servers = store
        .servers()?
        .into_iter()
        .filter(|server| !server.error.is_empty())
        .map(|server| server.id)
        .collect::<std::collections::BTreeSet<_>>();
    Ok(store
        .feeds()?
        .into_iter()
        .filter(|feed| due(feed, &health, &retries, now))
        .filter(|feed| {
            feed.server.as_ref().is_none_or(|server| {
                !failed_servers.contains(&server.server_id)
            })
        })
        .collect())
}

pub(super) fn domain(address: &str) -> String {
    url::Url::parse(address)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| address.to_owned())
}

fn due(
    feed: &Feed,
    health: &BTreeMap<i64, i64>,
    retries: &BTreeMap<String, i64>,
    now: i64,
) -> bool {
    health.get(&feed.id).is_none_or(|retry_at| *retry_at <= now)
        && !retries.contains_key(&domain(&feed.url))
        && feed
            .server
            .as_ref()
            .is_none_or(|server| server.enabled && server.present)
}

fn commit(store: &mut Store, downloaded: Download) -> Result<bool> {
    let feed = downloaded.feed;
    let now = chrono::Utc::now().timestamp();
    store.record_domain_retry(&domain(&feed.url), downloaded.retry_at, now)?;
    let result = downloaded.result.and_then(|parsed| {
        if let Some(parsed) = parsed {
            // Missing publisher titles fall back to the URL during parsing.
            // Retain an existing human label instead of replacing it with that
            // fallback when a formerly failing subscription becomes readable.
            let title = if parsed.title == feed.url {
                &feed.title
            } else {
                &parsed.title
            };
            if let Some(remote) = parsed.remote.as_ref() {
                store.ingest_collected_remote(
                    &feed,
                    title,
                    &parsed.articles,
                    remote,
                    now,
                )?;
                return Ok(false);
            } else {
                store.ingest(&feed, title, &parsed.articles)?;
            }
        }
        Ok(true)
    });
    match result {
        Ok(true) => store.record_source_attempt(feed.id, now, "", 0)?,
        Ok(false) => {}
        Err(error) => {
            return store.record_collection_failure(
                &feed,
                now,
                &format!("{error:#}"),
                downloaded.retry_at,
            );
        }
    }

    Ok(false)
}

mod discovery;

#[cfg(test)]
mod tests;
