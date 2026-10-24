//! Deep adjacent pages verify stable IDs and totals against direct OFFSET reads.
use anyhow::{Context, Result, ensure};
use lume::{
    model::{ArticleQuery, FeedOrigin, IncomingArticle, View},
    store::Store,
};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("pages.sqlite3");
    let count = if stress { 100_000 } else { 10_000 };
    let store = fixture(&path, count)?;
    let reference = rusqlite::Connection::open(&path)?;
    let offset = count - 1000;
    let query = |offset| ArticleQuery {
        view: View::All,
        text: String::new(),
        offset,
        limit: 100,
        retained: None,
    };
    println!("deep pagination · articles={count} · offset={offset}");
    reference_pages(&reference, count, offset, iterations)?;
    super::measure("cached totals + adjacent keyset pages", iterations, || {
        for offset in [offset, offset + 100, offset + 200, offset + 100] {
            let page = store.query_articles(&query(offset))?;
            ensure!(
                page.total == count && page.articles.len() == 100,
                "Page bounds"
            );
            ensure!(
                page.articles.first().context("First article")?.id
                    == (count - offset) as i64,
                "Cursor drift"
            );
        }
        Ok(())
    })
}

fn reference_pages(
    reference: &rusqlite::Connection,
    count: usize,
    offset: usize,
    iterations: usize,
) -> Result<()> {
    let sql = format!(
        "WITH candidates AS ({}), matching AS MATERIALIZED ({})\n{}\n{}",
        include_str!("../../src/store/sql/query/benchmark_candidates.sql"),
        include_str!("../../src/store/sql/query/benchmark_offset.sql"),
        include_str!("../../src/store/sql/select_article_metadata.sql"),
        include_str!("../../src/store/sql/select_page.sql")
    );
    super::measure(
        "OFFSET + exact recount + metadata reference",
        iterations,
        || {
            for offset in [offset, offset + 100, offset + 200, offset + 100] {
                let total: usize = reference.query_row(
                    include_str!(
                        "../../src/store/sql/query/benchmark_count.sql"
                    ),
                    [],
                    |row| row.get(0),
                )?;
                let ids = reference
                    .prepare_cached(&sql)?
                    .query_map([offset], |row| {
                        let article = metadata(row)?;
                        Ok(article.id)
                    })?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                ensure!(
                    ids.first() == Some(&((count - offset) as i64))
                        && ids.len() == 100
                        && total == count,
                    "Reference drift"
                );
            }
            Ok(())
        },
    )?;
    Ok(())
}

pub(super) fn fixture(path: &std::path::Path, count: usize) -> Result<Store> {
    fixture_with_body(path, count, "Systems body")
}

pub(super) fn fixture_with_body(
    path: &std::path::Path,
    count: usize,
    body: &str,
) -> Result<Store> {
    let mut store = Store::open(path)?;
    store.subscribe("https://pages.example/feed")?;
    let feed = store.feeds()?.remove(0);
    for start in (0..count).step_by(2000) {
        let articles = (start..(start + 2000).min(count))
            .map(|index| IncomingArticle {
                external_id: index.to_string(),
                title: format!("Systems {index}"),
                url: format!("https://pages.example/{index}"),
                author: String::new(),
                published: index as i64,
                body: body.into(),
                body_origin: FeedOrigin::Feed,
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, "Pages", &articles)?;
    }
    Ok(store)
}

fn metadata(row: &rusqlite::Row<'_>) -> rusqlite::Result<lume::model::Article> {
    Ok(lume::model::Article {
        id: row.get(0)?,
        feed_id: row.get(1)?,
        title: row.get(2)?,
        url: row.get(3)?,
        feed: row.get(4)?,
        author: row.get(5)?,
        published: row.get(6)?,
        unread: row.get(7)?,
        starred: row.get(8)?,
        comments: row.get(9)?,
        queued: row.get(10)?,
    })
}
