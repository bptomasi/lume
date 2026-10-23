//! Controlled download boundaries qualify actual worker overlap and deferral.

use std::sync::{Mutex, atomic::AtomicUsize};

use anyhow::{Context, Result};

use super::*;

fn library(urls: &[&str]) -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("collector.sqlite3"))?;
    for url in urls {
        store.subscribe(url)?;
    }
    Ok((directory, store))
}

#[test]
fn a_missing_publisher_title_preserves_the_existing_subscription_label()
-> Result<()> {
    let (_directory, mut store) = library(&["https://journal.example/feed"])?;
    let feed = store.feeds()?.remove(0);
    store.ingest(&feed, "Personal journal label", &[])?;
    let feed = store.feeds()?.remove(0);
    let fixture = include_str!("../../../tests/fixtures/rss.xml")
        .replace("<title>Systems Journal</title>", "");
    let parsed = crate::source::parse(fixture.as_bytes(), &feed.url)?;
    assert_eq!(parsed.title, feed.url);

    let failed = commit(
        &mut store,
        Download {
            feed,
            result: Ok(Some(parsed)),
            retry_at: 0,
        },
    )?;

    assert!(!failed);
    assert_eq!(store.feeds()?[0].title, "Personal journal label");
    assert_eq!(store.library()?.articles.len(), 1);
    Ok(())
}

#[test]
fn four_domains_overlap_but_never_exceed_four_active_downloads() -> Result<()> {
    let (_directory, mut store) = library(&[
        "https://a.example/feed",
        "https://b.example/feed",
        "https://c.example/feed",
        "https://d.example/feed",
    ])?;
    let (probe, supervisor) = OverlapProbe::start();

    let result =
        collect(&mut store, &AtomicBool::new(false), |_, _| {}, &|feed| {
            probe.fetch(feed)
        })?;

    supervisor
        .join()
        .map_err(|_| anyhow::anyhow!("Supervisor panicked"))??;
    assert!(matches!(
        result,
        Progress::Finished {
            feeds: 4,
            failures: 0
        }
    ));
    assert_eq!(probe.maximum.load(Ordering::SeqCst), 4);
    assert!(
        store
            .source_health()?
            .iter()
            .all(|source| source.succeeded_at > 0)
    );
    Ok(())
}

#[test]
fn retry_after_defers_same_domain_and_survives_the_next_collection()
-> Result<()> {
    let (_directory, mut store) =
        library(&["https://a.example/one", "https://a.example/two"])?;
    let calls = AtomicUsize::new(0);
    let deadline = chrono::Utc::now().timestamp() + 3600;
    let fetch = |feed: &Feed| {
        calls.fetch_add(1, Ordering::SeqCst);
        Download {
            feed: feed.clone(),
            result: Err(anyhow::anyhow!("HTTP 429")),
            retry_at: deadline,
        }
    };

    let first =
        collect(&mut store, &AtomicBool::new(false), |_, _| {}, &fetch)?;
    let second =
        collect(&mut store, &AtomicBool::new(false), |_, _| {}, &fetch)?;

    assert!(matches!(
        first,
        Progress::Finished {
            feeds: 2,
            failures: 2
        }
    ));
    assert!(matches!(
        second,
        Progress::Finished {
            feeds: 0,
            failures: 0
        }
    ));
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(
        store
            .source_health()?
            .iter()
            .all(|source| source.retry_at >= deadline)
    );
    Ok(())
}

#[test]
fn cancellation_during_download_prevents_a_later_health_or_content_write()
-> Result<()> {
    let (_directory, mut store) = library(&["https://a.example/feed"])?;
    let stopped = AtomicBool::new(false);
    let fetch = |feed: &Feed| {
        stopped.store(true, Ordering::Relaxed);
        Download {
            feed: feed.clone(),
            result: Ok(None),
            retry_at: 0,
        }
    };

    collect(&mut store, &stopped, |_, _| {}, &fetch)?;

    let health = store
        .source_health()?
        .pop()
        .context("Missing subscription")?;
    assert_eq!(health.attempted_at, 0);
    assert!(store.library()?.articles.is_empty());
    Ok(())
}

#[test]
fn cancellation_after_one_commit_does_not_drain_queued_results_into_storage()
-> Result<()> {
    let (_directory, mut store) = library(&[
        "https://a.example/feed",
        "https://b.example/feed",
        "https://c.example/feed",
        "https://d.example/feed",
    ])?;
    let stopped = AtomicBool::new(false);
    let fetch = |feed: &Feed| Download {
        feed: feed.clone(),
        result: Ok(None),
        retry_at: 0,
    };

    let result = collect(
        &mut store,
        &stopped,
        |_, _| stopped.store(true, Ordering::Relaxed),
        &fetch,
    )?;

    assert!(matches!(
        result,
        Progress::Finished {
            feeds: 1,
            failures: 0
        }
    ));
    assert_eq!(
        store
            .source_health()?
            .iter()
            .filter(|source| source.attempted_at > 0)
            .count(),
        1
    );
    Ok(())
}

struct OverlapProbe {
    active: AtomicUsize,
    maximum: AtomicUsize,
    started: mpsc::Sender<()>,
    released: Mutex<mpsc::Receiver<()>>,
}

impl OverlapProbe {
    fn start() -> (Self, thread::JoinHandle<Result<()>>) {
        let (started, starts) = mpsc::channel();
        let (release, released) = mpsc::channel();
        let released = Mutex::new(released);
        let active = AtomicUsize::new(0);
        let maximum = AtomicUsize::new(0);
        let supervisor = thread::spawn(move || -> Result<()> {
            for _ in 0..4 {
                starts.recv_timeout(Duration::from_secs(3))?;
            }
            for _ in 0..4 {
                release.send(())?;
            }
            Ok(())
        });

        (
            Self {
                active,
                maximum,
                started,
                released,
            },
            supervisor,
        )
    }

    fn fetch(&self, feed: &Feed) -> Download {
        let count = self.active.fetch_add(1, Ordering::SeqCst) + 1;
        self.maximum.fetch_max(count, Ordering::SeqCst);
        let result = (|| -> Result<_> {
            self.started.send(())?;
            self.released
                .lock()
                .map_err(|_| anyhow::anyhow!("Release lock poisoned"))?
                .recv_timeout(Duration::from_secs(3))?;
            Ok(None)
        })();
        self.active.fetch_sub(1, Ordering::SeqCst);
        Download {
            feed: feed.clone(),
            result,
            retry_at: 0,
        }
    }
}

#[test]
fn retry_after_survives_restart_for_a_new_feed_on_the_same_hostname()
-> Result<()> {
    let (directory, mut store) = library(&["https://a.example/one"])?;
    let deadline = chrono::Utc::now().timestamp() + 3600;
    collect(&mut store, &AtomicBool::new(false), |_, _| {}, &|feed| {
        Download {
            feed: feed.clone(),
            result: Err(anyhow::anyhow!("HTTP 429")),
            retry_at: deadline,
        }
    })?;
    store.subscribe("https://a.example/new")?;
    drop(store);
    let mut reopened =
        Store::open(&directory.path().join("collector.sqlite3"))?;
    let calls = AtomicUsize::new(0);

    let result =
        collect(&mut reopened, &AtomicBool::new(false), |_, _| {}, &|feed| {
            calls.fetch_add(1, Ordering::SeqCst);
            Download {
                feed: feed.clone(),
                result: Ok(None),
                retry_at: 0,
            }
        })?;

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(matches!(result, Progress::Finished { feeds: 0, .. }));
    assert_eq!(
        reopened.domain_retries(0)?.get("a.example"),
        Some(&deadline)
    );
    Ok(())
}

#[test]
fn ordinary_feed_backoff_does_not_cool_down_other_feeds_on_the_hostname()
-> Result<()> {
    let (_directory, mut store) = library(&["https://a.example/one"])?;
    collect(&mut store, &AtomicBool::new(false), |_, _| {}, &|feed| {
        Download {
            feed: feed.clone(),
            result: Err(anyhow::anyhow!("HTTP 404")),
            retry_at: 0,
        }
    })?;
    store.subscribe("https://a.example/other")?;
    let calls = AtomicUsize::new(0);

    collect(&mut store, &AtomicBool::new(false), |_, _| {}, &|feed| {
        calls.fetch_add(1, Ordering::SeqCst);
        Download {
            feed: feed.clone(),
            result: Ok(None),
            retry_at: 0,
        }
    })?;

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(store.domain_retries(0)?.is_empty());
    Ok(())
}

#[test]
fn hostname_retry_keeps_the_longest_deadline_and_ignores_expired_entries()
-> Result<()> {
    let (_directory, store) = library(&["https://a.example/feed"])?;
    store.record_domain_retry("a.example", 300, 100)?;
    store.record_domain_retry("a.example", 200, 100)?;
    store.record_domain_retry("b.example", 110, 100)?;
    assert_eq!(store.domain_retries(110)?.len(), 1);
    assert_eq!(store.domain_retries(110)?.get("a.example"), Some(&300));
    assert!(store.domain_retries(300)?.is_empty());
    Ok(())
}

#[test]
fn a_server_discovery_failure_does_not_retry_each_remote_feed() -> Result<()> {
    let (_directory, mut store) = library(&["https://local.example/feed"])?;
    let server = store.add_server("https://reader.example", "LUME_TOKEN")?;
    store.sync_server_feeds(
        server,
        &[crate::model::RemoteFeedCandidate {
            remote_id: 7,
            title: "Remote".into(),
            remote_url: "https://publisher.example/feed".into(),
            local_url: "https://reader.example/v1/feeds/7".into(),
        }],
    )?;
    store.record_server_attempt(server, 100, "Authentication failed")?;
    let eligible = eligible_feeds(&store)?;
    assert_eq!(eligible.len(), 1);
    assert_eq!(eligible[0].url, "https://local.example/feed");
    store.record_server_attempt(server, 101, "")?;
    assert_eq!(eligible_feeds(&store)?.len(), 2);
    Ok(())
}
