//! Global relevance with common terms, old titles and bounded metadata pages.

use anyhow::{Result, ensure};
use lume::{
    model::{ArticleQuery, FeedOrigin, IncomingArticle, View},
    store::Store,
};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 20_000 } else { 2000 };
    let (_directory, store) = library(count)?;
    let mut query = ArticleQuery {
        view: View::All,
        text: "Why Is My Kubernetes".into(),
        offset: 0,
        limit: 20,
        retained: None,
    };
    let target = store.query_articles(&query)?.articles[0].id;
    println!("relevance input · articles={count} · matches={count}");
    super::measure("ranked title search + bounded page", iterations, || {
        let page = store.query_articles(&query)?;
        ensure!(page.total == count, "Ranked membership changed");
        ensure!(page.articles.len() == 20, "Metadata page is not bounded");
        ensure!(
            page.articles[0].title == "Why Is My Kubernetes Job Pending?",
            "Old title must outrank newer body matches"
        );
        Ok(())
    })?;
    query.offset = count - 20;
    super::measure(
        "ranked identity jump from the last page",
        iterations,
        || {
            let (offset, page) = store
                .page_containing(&query, target)?
                .ok_or_else(|| anyhow::anyhow!("Ranked target missing"))?;
            ensure!(
                offset == 0 && page.articles[0].id == target,
                "Identity jump used a different order"
            );
            Ok(())
        },
    )
}

fn library(count: usize) -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("relevance.sqlite3"))?;
    store.subscribe("https://relevance.example/feed")?;
    let feed = store.feeds()?.remove(0);
    for start in (0..count).step_by(1000) {
        let articles = (start..(start + 1000).min(count))
            .map(article)
            .collect::<Vec<_>>();
        store.ingest(&feed, "Engineering journal", &articles)?;
    }
    Ok((directory, store))
}

fn article(index: usize) -> IncomingArticle {
    IncomingArticle {
        external_id: index.to_string(),
        title: if index == 0 {
            "Why Is My Kubernetes Job Pending?".into()
        } else {
            format!("Engineering journal {index}")
        },
        url: format!("https://relevance.example/{index}"),
        author: "Fixture".into(),
        published: 1_790_000_000 + index as i64,
        body: "Why is my Kubernetes job pending? ".to_owned()
            + &include_str!("../../src/demo/systems.txt").repeat(2),
        body_origin: FeedOrigin::Feed,
    }
}
