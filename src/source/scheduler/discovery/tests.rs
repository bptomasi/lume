//! Controlled discovery delays verify progress without timing-based speed claims.
use super::*;
use std::sync::Mutex;

#[test]
fn slow_discovery_does_not_hold_fast_servers_or_publishers() -> Result<()> {
    let (_directory, mut store) = discovery_library()?;
    let (release, wait) = mpsc::sync_channel(1);
    let wait = Mutex::new(wait);
    let (entered, observed) = mpsc::sync_channel(1);
    let result = collect(
        &mut store,
        &AtomicBool::new(false),
        |visited, _| {
            if visited == 2 {
                let _ = release.try_send(());
            }
        },
        &|server| {
            if server.url.contains("a-slow") {
                entered.send(())?;
                wait.lock()
                    .map_err(|_| anyhow::anyhow!("Discovery gate"))?
                    .recv_timeout(Duration::from_secs(5))?;
                Ok(Vec::new())
            } else {
                Ok(vec![candidate(server)])
            }
        },
        &unchanged,
    )?;
    observed.recv_timeout(Duration::from_secs(1))?;
    assert!(matches!(
        result,
        Progress::Finished {
            feeds: 2,
            failures: 0
        }
    ));
    let successes = store
        .source_health()?
        .iter()
        .filter(|source| source.succeeded_at > 0)
        .count();
    assert_eq!(successes, 2);
    Ok(())
}

fn candidate(server: &Server) -> RemoteFeedCandidate {
    RemoteFeedCandidate {
        remote_id: 7,
        title: "Remote".into(),
        remote_url: "https://remote.example/feed".into(),
        local_url: format!("{}/v1/feeds/7", server.url),
    }
}

#[test]
fn late_discovery_preserves_a_changed_connection_and_its_feed_set() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("sources.sqlite3");
    let mut store = Store::open(&path)?;
    let id = store.add_server("https://old.example", "TOKEN")?;
    let server = store.servers()?.remove(0);
    let other = Store::open(&path)?;
    other.remove_server(id)?;
    let (feeds, failed) = publish(
        &mut store,
        &server,
        Ok(vec![candidate(&server)]),
        &BTreeMap::new(),
    )?;
    assert!(feeds.is_empty());
    assert!(!failed);
    assert!(store.feeds()?.is_empty());
    assert!(store.servers()?.is_empty());
    Ok(())
}

fn unchanged(feed: &Feed) -> Download {
    Download {
        feed: feed.clone(),
        result: Ok(None),
        retry_at: 0,
    }
}

fn discovery_library() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("sources.sqlite3"))?;
    store.add_server("https://a-slow.example", "TOKEN")?;
    store.add_server("https://b-fast.example", "TOKEN")?;
    store.subscribe("https://c-publisher.example/feed")?;
    Ok((directory, store))
}
