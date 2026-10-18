//! Four bounded download lanes, with one lane per domain and one SQLite writer.
//!
//! Domain groups never overlap across lanes. A lane spaces starts by 500 ms
//! and carries Retry-After forward to its remaining feeds. Results cross a
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
    collect(&mut store, stopped, progress, &|feed| {
        download::fetch(client, feed)
    })
}

fn collect(
    store: &mut Store,
    stopped: &AtomicBool,
    progress: impl Fn(usize, usize),
    fetch: &(impl Fn(&Feed) -> Download + Sync),
) -> Result<Progress> {
    let now = chrono::Utc::now().timestamp();
    let retries = store.domain_retries(now)?;
    let health = store
        .source_health()?
        .into_iter()
        .map(|source| (source.feed, source.retry_at))
        .collect::<BTreeMap<_, _>>();
    let eligible = store
        .feeds()?
        .into_iter()
        .filter(|feed| {
            health.get(&feed.id).is_none_or(|retry_at| *retry_at <= now)
                && !retries.contains_key(&domain(&feed.url))
        })
        .collect::<Vec<_>>();
    let total = eligible.len();
    let buckets = buckets(eligible);
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::sync_channel(4);
        for bucket in buckets {
            let sender = sender.clone();
            scope.spawn(move || lane(bucket, stopped, sender, fetch));
        }
        drop(sender);
        let mut visited = 0;
        let mut failures = 0;
        for downloaded in receiver {
            if stopped.load(Ordering::Relaxed) {
                break;
            }
            visited += 1;
            failures += usize::from(commit(store, downloaded)?);
            progress(visited, total);
        }
        Ok(Progress::Finished {
            feeds: visited,
            failures,
        })
    })
}

fn buckets(feeds: Vec<Feed>) -> Vec<Vec<Feed>> {
    let mut domains: BTreeMap<String, Vec<Feed>> = BTreeMap::new();
    for feed in feeds {
        domains.entry(domain(&feed.url)).or_default().push(feed);
    }
    let mut buckets: Vec<Vec<Feed>> = (0..4).map(|_| Vec::new()).collect();
    for group in domains.into_values() {
        if let Some(bucket) =
            buckets.iter_mut().min_by_key(|bucket| bucket.len())
        {
            bucket.extend(group);
        }
    }
    buckets
}

fn domain(address: &str) -> String {
    url::Url::parse(address)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| address.to_owned())
}

fn lane(
    feeds: Vec<Feed>,
    stopped: &AtomicBool,
    sender: mpsc::SyncSender<Download>,
    fetch: &(impl Fn(&Feed) -> Download + Sync),
) {
    let mut starts: BTreeMap<String, Instant> = BTreeMap::new();
    let mut deadlines: BTreeMap<String, i64> = BTreeMap::new();
    for feed in feeds {
        if stopped.load(Ordering::Relaxed) {
            return;
        }
        let host = domain(&feed.url);
        if let Some(previous) = starts.get(&host) {
            thread::sleep(
                Duration::from_millis(500).saturating_sub(previous.elapsed()),
            );
        }
        if stopped.load(Ordering::Relaxed) {
            return;
        }
        starts.insert(host.clone(), Instant::now());
        let deadline = deadlines.get(&host).copied().unwrap_or(0);
        let downloaded = if deadline > chrono::Utc::now().timestamp() {
            Download {
                feed,
                result: Err(anyhow::anyhow!(
                    "Domain Retry-After until {deadline}"
                )),
                retry_at: deadline,
            }
        } else {
            fetch(&feed)
        };
        if downloaded.retry_at > 0 {
            deadlines.insert(host, downloaded.retry_at);
        }
        if stopped.load(Ordering::Relaxed) || sender.send(downloaded).is_err() {
            return;
        }
    }
}

fn commit(store: &mut Store, downloaded: Download) -> Result<bool> {
    let feed = downloaded.feed;
    let now = chrono::Utc::now().timestamp();
    store.record_domain_retry(&domain(&feed.url), downloaded.retry_at, now)?;
    let result = downloaded.result.and_then(|parsed| {
        if let Some(parsed) = parsed {
            store.ingest(&feed, &parsed.title, &parsed.articles)?;
        }
        Ok(())
    });
    match result {
        Ok(()) => store.record_source_attempt(feed.id, now, "", 0)?,
        Err(error) => {
            let failures = store.source_failures(feed.id)?;
            let delay = 60_i64.saturating_mul(1_i64 << failures.min(10));
            let retry_at = downloaded.retry_at.max(now.saturating_add(delay));
            store.record_source_attempt(
                feed.id,
                now,
                &format!("{error:#}"),
                retry_at,
            )?;
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
