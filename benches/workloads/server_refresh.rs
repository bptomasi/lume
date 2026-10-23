//! Routine server refreshes must not rebuild unchanged publisher indexes.

use anyhow::{Context, Result, ensure};
use lume::{
    model::{
        FeedOrigin, IncomingArticle, RemoteFeedCandidate, RemoteFeedUpdate,
        View,
    },
    store::Store,
};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("refresh.sqlite3"))?;
    let count = if stress { 1000 } else { 100 };
    let server = store.add_server("https://reader.example", "TOKEN")?;
    let candidates = candidates(count);
    store.sync_server_feeds(server, &candidates)?;
    let feeds = store.feeds()?;
    let articles = articles(2000);
    let first = feeds.first().context("Seed feed")?;
    let remote = RemoteFeedUpdate {
        server_id: server,
        remote_id: 1,
        initialized: true,
        last_changed: 100,
        sync_after_id: 0,
        sync_latest: 100,
        states: Default::default(),
    };
    store.ingest_remote(first, &first.title, &articles, &remote)?;
    let id = store.library()?.articles[0].id;
    store.save_comment(id, None, "Keep this personal note")?;
    store.set_starred(id, true)?;
    discovery(&mut store, server, &candidates, iterations)?;
    super::measure("unchanged remote page (2,000 entries)", iterations, || {
        store.ingest_remote(first, &first.title, &articles, &remote)?;
        ensure!(
            store.view_count(&View::Server(server))? == 2000,
            "Entries changed"
        );
        ensure!(store.article(id)?.starred, "Personal favorite lost");
        ensure!(store.comments(id)?.len() == 1, "Personal note lost");
        Ok(())
    })
}

fn discovery(
    store: &mut Store,
    server: i64,
    candidates: &[RemoteFeedCandidate],
    iterations: usize,
) -> Result<()> {
    super::measure(
        "unchanged server discovery (2,000 cached articles)",
        iterations,
        || {
            store.sync_server_feeds(server, candidates)?;
            ensure!(
                store.feeds()?.len() == candidates.len(),
                "Feed identity changed"
            );
            Ok(())
        },
    )
}

fn candidates(count: usize) -> Vec<RemoteFeedCandidate> {
    (1..=count)
        .map(|index| RemoteFeedCandidate {
            remote_id: index as i64,
            title: format!("Remote {index:04}"),
            remote_url: format!("https://publisher.example/feed/{index}"),
            local_url: format!("https://reader.example/v1/feeds/{index}"),
        })
        .collect()
}

fn articles(count: usize) -> Vec<IncomingArticle> {
    (0..count)
        .map(|index| IncomingArticle {
            external_id: format!("miniflux:{}", index + 1),
            title: format!("Remote systems article {index}"),
            url: format!("https://publisher.example/article/{index}"),
            author: "Publisher".into(),
            published: 1_800_000_000 + index as i64,
            body: "Systems, queues and resource scheduling. ".repeat(100),
            body_origin: FeedOrigin::Feed,
        })
        .collect()
}
