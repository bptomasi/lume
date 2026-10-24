//! Discovery and entries share bounded domain lanes and one SQLite publisher.
use super::*;
use crate::model::{RemoteFeedCandidate, Server};

mod lane;

pub(super) enum Job {
    Publisher(Feed),
    Server(Server),
}

enum Event {
    Download(Box<Download>),
    Discovery {
        server: Server,
        result: Result<Vec<RemoteFeedCandidate>>,
        reply: mpsc::SyncSender<Vec<Feed>>,
    },
}

pub(super) fn collect(
    store: &mut Store,
    stopped: &AtomicBool,
    progress: impl Fn(usize, usize),
    discover: &(impl Fn(&Server) -> Result<Vec<RemoteFeedCandidate>> + Sync),
    fetch: &(impl Fn(&Feed) -> Download + Sync),
) -> Result<Progress> {
    let health = store
        .source_health()?
        .into_iter()
        .map(|source| (source.feed, source.retry_at))
        .collect();
    let jobs = jobs(store, &health)?;
    let total = jobs.iter().map(Vec::len).sum();
    if total == 0 {
        return Ok(Progress::Finished {
            feeds: 0,
            failures: 0,
        });
    }
    let lanes = jobs.len().min(4);
    let jobs = std::sync::Mutex::new(std::collections::VecDeque::from(jobs));
    thread::scope(|scope| {
        let (sender, receiver) = mpsc::sync_channel(4);
        for _ in 0..lanes {
            let sender = sender.clone();
            let jobs = &jobs;
            scope.spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    let group = match jobs.lock() {
                        Ok(mut jobs) => jobs.pop_front(),
                        Err(_) => return,
                    };
                    let Some(group) = group else {
                        return;
                    };
                    lane::run(group, stopped, sender.clone(), discover, fetch);
                }
            });
        }
        drop(sender);
        receive(store, receiver, stopped, total, &health, &progress)
    })
}

fn jobs(store: &Store, health: &BTreeMap<i64, i64>) -> Result<Vec<Vec<Job>>> {
    let mut domains: BTreeMap<String, Vec<Job>> = BTreeMap::new();
    let now = chrono::Utc::now().timestamp();
    let retries = store.domain_retries(now)?;
    for feed in store
        .publisher_feeds()?
        .into_iter()
        .filter(|feed| due(feed, health, &retries, now))
    {
        domains
            .entry(domain(&feed.url))
            .or_default()
            .push(Job::Publisher(feed));
    }
    for server in store.servers()?.into_iter().filter(|server| server.enabled) {
        let host = domain(&server.url);
        if !retries.contains_key(&host) {
            domains.entry(host).or_default().push(Job::Server(server));
        }
    }
    Ok(domains.into_values().collect())
}

fn receive(
    store: &mut Store,
    receiver: mpsc::Receiver<Event>,
    stopped: &AtomicBool,
    mut total: usize,
    health: &BTreeMap<i64, i64>,
    progress: &impl Fn(usize, usize),
) -> Result<Progress> {
    let (mut visited, mut failures) = (0, 0);
    for event in receiver {
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        match event {
            Event::Download(downloaded) => {
                failures += usize::from(commit(store, *downloaded)?);
                visited += 1;
            }
            Event::Discovery {
                server,
                result,
                reply,
            } => {
                let (feeds, failed) = publish(store, &server, result, health)?;
                failures += usize::from(failed);
                total = total.saturating_sub(1).saturating_add(feeds.len());
                // A disconnected lane needs no reply; its connection was never
                // used to write or read SQLite from a download thread.
                let _ = reply.send(feeds);
            }
        }
        progress(visited, total);
    }
    Ok(Progress::Finished {
        feeds: visited,
        failures,
    })
}

fn publish(
    store: &mut Store,
    server: &Server,
    result: Result<Vec<RemoteFeedCandidate>>,
    health: &BTreeMap<i64, i64>,
) -> Result<(Vec<Feed>, bool)> {
    let now = chrono::Utc::now().timestamp();
    match result {
        Ok(candidates)
            if store.sync_collected_server_feeds(
                server,
                &candidates,
                now,
            )? =>
        {
            let retries = store.domain_retries(now)?;
            let feeds = store
                .server_feeds(server.id)?
                .into_iter()
                .filter(|feed| due(feed, health, &retries, now))
                .collect();
            Ok((feeds, false))
        }
        Ok(_) => Ok((Vec::new(), false)),
        Err(error) => {
            let retry_at = super::super::miniflux::retry_deadline(&error);
            store.record_domain_retry(&domain(&server.url), retry_at, now)?;
            let failed = store.record_collected_server_failure(
                server,
                now,
                &format!("{error:#}"),
            )?;
            Ok((Vec::new(), failed))
        }
    }
}

#[cfg(test)]
mod tests;
