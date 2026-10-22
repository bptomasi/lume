//! Representative metadata plans, offline packages and cache maintenance.

use std::{
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use lume::{
    model::{
        ArticleTarget, ExtractedDocument, OfflineLimits, OfflineStatus,
        SessionItem, View,
    },
    store::Store,
};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    plans(iterations, stress)?;
    packages(iterations, stress)?;
    cleanup(iterations, stress)
}

fn plans(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 512 } else { 100 };
    let candidates = (1..=count)
        .map(|id| SessionItem {
            article: id,
            feed: id % 10,
            url: format!("https://example.org/{id}"),
            canonical: format!("https://example.org/{}", id / 2),
            title: format!("Candidate {id}"),
            minutes: (id % 15 + 1) as u16,
            score: 100 + id % 50,
            topic: format!("Topic {}", id % 7),
        })
        .collect::<Vec<_>>();
    super::measure(
        "time-budget plan: 100/512 metadata candidates",
        iterations * 20,
        || {
            let session =
                lume::recommendation::session::plan(candidates.clone(), 30)?;
            ensure!(!session.items.is_empty(), "Invalid session size");
            let keys = session
                .items
                .iter()
                .map(|item| &item.canonical)
                .collect::<std::collections::BTreeSet<_>>();
            ensure!(
                keys.len() == session.items.len(),
                "Duplicate session article"
            );
            ensure!(
                session.items.iter().map(|item| item.minutes).sum::<u16>()
                    <= 30,
                "Budget exceeded"
            );
            Ok(())
        },
    )
}

pub(super) fn fixture(count: usize) -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let id = store.subscribe("https://example.org/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .ok_or_else(|| anyhow::anyhow!("Missing feed"))?;
    let mut parsed = lume::source::parse(
        include_bytes!("../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    let template = parsed.articles.remove(0);
    let articles = (0..count)
        .map(|index| lume::model::IncomingArticle {
            external_id: format!("service-{index}"),
            url: format!("https://example.org/{index}"),
            title: template.title.clone(),
            author: template.author.clone(),
            published: template.published + index as i64,
            body: "Publisher feed remains. ".repeat(50),
            body_origin: template.body_origin,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, "Benchmark", &articles)?;
    Ok((directory, store))
}

fn fill(store: &Store, queued: bool, images: bool) -> Result<usize> {
    let articles = store
        .query_articles(&lume::model::ArticleQuery {
            view: View::All,
            text: String::new(),
            offset: 0,
            limit: 100,
            retained: None,
        })?
        .articles;
    for article in &articles {
        let mut body = "Representative full article. ".repeat(100);
        if images {
            body.push_str("\n\n![Diagram](https://example.org/diagram.svg)\n");
        }
        store.cache_document(
            &ArticleTarget {
                article: article.id,
                url: article.url.clone(),
            },
            &ExtractedDocument {
                final_url: article.url.clone(),
                body,
            },
        )?;
        store.set_queued(article.id, queued)?;
    }
    Ok(articles.len())
}

fn packages(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 100 } else { 10 };
    let (directory, store) = fixture(count)?;
    fill(&store, true, true)?;
    let images = directory.path().join("images");
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="160" height="90"><rect width="160" height="90" fill="orange"/></svg>"#;
    lume::media::cache::write(&images, "https://example.org/diagram.svg", svg)?;
    let captured =
        lume::offline::capture(&store, View::Queue, OfflineLimits::default())?;
    super::measure(
        "offline package: cached text + validated/pinned SVG",
        iterations,
        || {
            let state = lume::offline::prepare(
                &store,
                &images,
                captured.clone(),
                &AtomicBool::new(false),
                |_| {},
            )?;
            ensure!(state.items.len() == count, "Package lost entries");
            ensure!(
                state
                    .items
                    .iter()
                    .all(|item| item.status == OfflineStatus::Ready
                        && item.images == 1),
                "Incomplete package"
            );
            ensure!(
                state.items.iter().map(|item| item.bytes).sum::<u64>()
                    <= state.limits.bytes,
                "Package exceeded budget"
            );
            Ok(())
        },
    )
}

fn cleanup(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 100 } else { 10 };
    let (directory, store) = fixture(count)?;
    let images = directory.path().join("images");
    let mut elapsed = Duration::ZERO;
    for _ in 0..iterations {
        fill(&store, false, false)?;
        let started = Instant::now();
        let report = lume::storage::clean(
            &store,
            &images,
            lume::storage::Cleanup::All,
            &AtomicBool::new(false),
        )?;
        elapsed += started.elapsed();
        ensure!(
            report.contains(&format!("Cleared {count} unprotected")),
            "Cleanup count differs"
        );
        ensure!(article_count(&store)? == count, "Cleanup removed articles");
    }
    println!(
        "cache cleanup · documents={count} · mean={:.3} ms · setup excluded",
        elapsed.as_secs_f64() * 1000.0 / iterations as f64
    );
    super::measure("storage report: SQLite + file usage", iterations, || {
        ensure!(
            lume::storage::inspect(&store, &images)?
                .contains(&format!("Articles: {count}")),
            "Wrong report count"
        );
        Ok(())
    })
}

fn article_count(store: &Store) -> Result<usize> {
    Ok(store
        .query_articles(&lume::model::ArticleQuery {
            view: View::All,
            text: String::new(),
            offset: 0,
            limit: 100,
            retained: None,
        })?
        .total)
}
