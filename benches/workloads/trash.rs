//! Reversible visibility changes and oldest-page access with mixed Trash state.

use anyhow::{Result, ensure};
use lume::{
    model::{ArticleQuery, IncomingArticle, View},
    store::Store,
};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 8000 } else { 2000 };
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("trash.db"))?;
    let target = prepare(&mut store, count)?;
    println!("trash input: {count} articles, {} deleted", count / 2);
    oldest_page(&store, count, iterations)?;
    lifecycle(&store, target, iterations)
}

fn oldest_page(store: &Store, count: usize, iterations: usize) -> Result<()> {
    let mut oldest = query(View::Trash, "mechanism");
    oldest.offset = count / 2 - 20;
    oldest.limit = 20;
    super::super::measure(
        "Trash FTS count + oldest matching page",
        iterations,
        || {
            let page = store.query_articles(&oldest)?;
            ensure!(page.total == count / 2, "Trash count drifted");
            ensure!(
                page.articles.len() == 20,
                "Old Trash articles inaccessible"
            );
            ensure!(
                page.articles[0].title == "Article 38",
                "Wrong Trash order"
            );
            ensure!(
                page.articles[19].title == "Article 0",
                "Oldest article lost"
            );
            Ok(())
        },
    )
}

fn lifecycle(store: &Store, target: i64, iterations: usize) -> Result<()> {
    super::super::measure(
        "delete + FTS visibility + restore + undo",
        iterations,
        || {
            ensure!(store.set_deleted(target, true)?, "Deletion was a no-op");
            ensure!(
                store.query_articles(&query(View::All, "uniquenote"))?.total
                    == 0,
                "Deleted note leaked"
            );
            ensure!(
                store
                    .query_articles(&query(View::Trash, "uniquenote"))?
                    .total
                    == 1,
                "Deleted note lost"
            );
            ensure!(store.set_deleted(target, false)?, "Restore failed");
            ensure!(
                store.undo()? && store.is_deleted(target)?,
                "Restore undo failed"
            );
            ensure!(
                store.undo()? && !store.is_deleted(target)?,
                "Deletion undo failed"
            );
            ensure!(
                store.comments(target)?[0].body == "uniquenote",
                "Note changed"
            );
            ensure!(store.article(target)?.starred, "Favorite changed");
            Ok(())
        },
    )
}

fn prepare(store: &mut Store, count: usize) -> Result<i64> {
    store.subscribe("https://benchmark.example/trash")?;
    let feed = store.feeds()?.remove(0);
    for start in (0..count).step_by(2000) {
        let articles = (start..(start + 2000).min(count))
            .map(|index| IncomingArticle {
                body_origin: lume::model::FeedOrigin::Feed,
                external_id: index.to_string(),
                title: format!("Article {index}"),
                url: String::new(),
                author: String::new(),
                published: 2_000_000_000 + index as i64,
                body: "A representative mechanism to study.".into(),
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, "Lifecycle benchmark", &articles)?;
    }
    let articles = store.library()?.articles;
    ensure!(articles.len() == count, "Fixture exceeds snapshot contract");
    let target = articles[0].id;
    store.save_comment(target, None, "uniquenote")?;
    store.set_starred(target, true)?;
    for (index, article) in articles.iter().enumerate() {
        if index % 2 == 1 {
            store.set_deleted(article.id, true)?;
        }
    }
    store.clear_history()?;
    Ok(target)
}

fn query(view: View, text: &str) -> ArticleQuery {
    ArticleQuery {
        view,
        text: text.into(),
        offset: 0,
        limit: 10,
        retained: None,
    }
}
