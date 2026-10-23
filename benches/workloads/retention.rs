//! Preview and bounded expiry over aged caches with checked personal protections.

use anyhow::{Context, Result, ensure};
use lume::{
    config::{
        self, Session,
        retention::{Age, Policy},
    },
    model::{FeedOrigin, IncomingArticle, SmartFilter, View},
    retention,
    store::Store,
};
use std::{path::Path, sync::atomic::AtomicBool, time::Instant};

const NOW: i64 = 1_800_000_000;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 10_000 } else { 1000 };
    let directory = tempfile::tempdir()?;
    let path = directory.path();
    let store = prepare(path, count)?;
    let images = path.join("images");
    super::measure(
        &format!("retention preview ({count} cached articles)"),
        iterations,
        || {
            let preview = retention::preview(&store, &images, NOW)?;
            ensure!(
                preview.articles == count / 2 - 3,
                "Protected read item lost"
            );
            ensure!(
                preview.full_texts == count / 2 - 2,
                "Protected unread cache lost"
            );
            Ok(())
        },
    )?;
    let started = Instant::now();
    let applied =
        retention::apply(&store, &images, NOW, &AtomicBool::new(false), None)?;
    let elapsed = started.elapsed();
    ensure!(
        applied.articles == count / 2 - 3,
        "Incorrect deletion count"
    );
    ensure!(applied.full_texts == count / 2 - 2, "Incorrect cache count");
    verify(&store, count)?;
    println!(
        "retention apply ({count} cached articles, batch=32): {:.3} ms (checked)",
        elapsed.as_secs_f64() * 1000.0
    );
    Ok(())
}

fn prepare(path: &Path, count: usize) -> Result<Store> {
    let database = path.join("library.sqlite3");
    let mut store = Store::open(&database)?;
    seed(&mut store, count)?;
    rusqlite::Connection::open(&database)?.execute_batch(include_str!(
        "../../src/store/sql/bench_setup_retention.sql"
    ))?;
    store.set_starred(1, true)?;
    store.set_queued(2, true)?;
    store.save_collection("Personal reading", Some(3))?;
    store.save_comment(
        4,
        None,
        "A private note must protect its full cache",
    )?;
    store.save_smart_list(
        "Keep query",
        &SmartFilter {
            text: "retentionkept".into(),
            ..Default::default()
        },
    )?;
    store.clear_history()?;
    let document = path.join("config.toml");
    let session = Session::open(&store, &document)?;
    let mut config = config::decode(&session.initial().document)?;
    config.retention.defaults = Policy {
        read_days: Age(Some(90)),
        full_text_days: Age(Some(30)),
        ..Default::default()
    };
    config.retention.max_articles_per_run = count as u32;
    std::fs::write(&document, config::encode(&config)?)?;
    Session::open(&store, &document)?;
    Ok(store)
}

fn seed(store: &mut Store, count: usize) -> Result<()> {
    store.subscribe("https://journal.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .next()
        .context("Benchmark feed")?;
    let entries = (1..=count)
        .map(|id| IncomingArticle {
            external_id: format!("retention:{id}"),
            title: if id == 5 {
                "retentionkept".into()
            } else {
                format!("Article {id}")
            },
            url: format!("https://journal.example/{id}"),
            author: "Author".into(),
            published: 1_000_000_000,
            body: "Durable original feed content for the retention workload. "
                .repeat(5),
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    for batch in entries.chunks(2000) {
        store.ingest(&feed, &feed.title, batch)?;
    }
    Ok(())
}

fn verify(store: &Store, count: usize) -> Result<()> {
    ensure!(
        store.view_count(&View::All)? == count / 2 + 3,
        "Incorrect library size"
    );
    for id in 1..=5 {
        ensure!(
            !store
                .cached_document(id)?
                .context("Protected cache")?
                .body
                .is_empty(),
            "Personal cache lost"
        );
    }
    ensure!(
        store
            .cached_document(6)?
            .context("Expired cache")?
            .body
            .is_empty(),
        "Expired cache retained"
    );
    ensure!(!store.body(6)?.is_empty(), "Original feed text lost");
    ensure!(store.article(7).is_err(), "Expired read item retained");
    ensure!(store.article(1)?.starred, "Favorite lost");
    ensure!(store.article(2)?.queued, "Queue item lost");
    ensure!(
        store.article_memberships(3)?.len() == 1,
        "Folder membership lost"
    );
    ensure!(store.comments(4)?.len() == 1, "Note lost");
    Ok(())
}
