//! Checked feed refreshes and reading checkpoints separate setup from timers.

use anyhow::{Context, Result, ensure};
use lume::{
    model::{
        ArticleTarget, ExtractedDocument, FeedOrigin, IncomingArticle, View,
    },
    store::Store,
};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("refresh.sqlite3"))?;
    let count = if stress { 2000 } else { 200 };
    store.subscribe("https://publisher.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let mut entries = entries(count);
    store.ingest(&feed, "Journal", &entries)?;
    let feed = store.feeds()?.remove(0);
    let id = store.library()?.articles[0].id;
    store.set_starred(id, true)?;
    store.save_comment(id, None, "Keep this private note")?;
    refreshes(&mut store, &feed, &mut entries, id, iterations)?;
    checkpoints(&store, iterations)
}

fn refreshes(
    store: &mut Store,
    feed: &lume::model::Feed,
    entries: &mut [IncomingArticle],
    id: i64,
    iterations: usize,
) -> Result<()> {
    let count = entries.len();
    super::measure(
        &format!("unchanged publisher refresh ({count} entries)"),
        iterations,
        || {
            store.ingest(feed, "Journal", entries)?;
            ensure!(
                store.view_count(&View::All)? == count,
                "Article count changed"
            );
            ensure!(store.article(id)?.starred, "Favorite lost");
            ensure!(store.comments(id)?.len() == 1, "Note lost");
            Ok(())
        },
    )?;
    let mut revision = 0;
    super::measure(
        &format!("one changed publisher article ({count} entries)"),
        iterations,
        || {
            revision += 1;
            entries[0].body =
                format!("Changed publisher content {revision}. ").repeat(100);
            store.ingest(feed, "Journal", entries)?;
            ensure!(
                store.body(1)? == entries[0].body,
                "Publisher correction lost"
            );
            ensure!(store.comments(id)?.len() == 1, "Note lost");
            Ok(())
        },
    )?;
    Ok(())
}

fn entries(count: usize) -> Vec<IncomingArticle> {
    (1..=count)
        .map(|id| IncomingArticle {
            external_id: format!("publisher:{id}"),
            title: format!("Publisher article {id}"),
            url: format!("https://publisher.example/article/{id}"),
            author: "Publisher".into(),
            published: 1_800_000_000 + id as i64,
            body: "Systems, queues and resource scheduling. ".repeat(100),
            body_origin: FeedOrigin::Feed,
        })
        .collect()
}

fn checkpoints(store: &Store, iterations: usize) -> Result<()> {
    let target = ArticleTarget {
        article: 1,
        url: store.article(1)?.url,
    };
    store.cache_document(
        &target,
        &ExtractedDocument {
            final_url: target.url.clone(),
            body: "Independent cached article text. ".repeat(200),
        },
    )?;
    let revision = store.cached_document(1)?.context("Full cache")?.revision;
    let workspace = store.workspace()?;
    let layouts = store.layouts(&workspace)?;
    store.save_layouts(&workspace, &layouts)?;
    store.save_progress(1, 42)?;
    store.save_document_progress(1, revision, 37)?;
    super::measure("100 identical reading checkpoints", iterations, || {
        for _ in 0..100 {
            store.save_layouts(&workspace, &layouts)?;
            store.save_progress(1, 42)?;
            store.save_document_progress(1, revision, 37)?;
        }
        ensure!(store.progress(1)? == 42, "Feed position lost");
        ensure!(
            store.cached_document(1)?.context("Full cache")?.progress == 37,
            "Full position lost"
        );
        ensure!(
            store.workspace()?.reading_width == workspace.reading_width,
            "Workspace changed"
        );
        Ok(())
    })
}
