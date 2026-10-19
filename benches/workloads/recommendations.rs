//! Checked incremental classification, cached navigation and feedback workloads.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    model::{
        ArticleQuery, Feed, FeedOrigin, IncomingArticle,
        RecommendationFeedback, View,
    },
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};
use std::time::Instant;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let verify = std::env::args().any(|argument| argument == "--test");
    let count = if verify {
        128
    } else if stress {
        50_000
    } else {
        10_000
    };
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("recommendations.sqlite3");
    let (mut store, feed) = seed(&path, count)?;
    println!(
        "recommendation input: {count} articles, ten sources, 64-row batches"
    );
    super::measure(
        "initial incremental classification + publication",
        1,
        || drain(&store),
    )?;
    let expected = (count - count / 3).min(100);
    cached(&store, iterations, expected)?;
    single_change(&mut store, &feed, iterations)?;
    feedback(&store)?;
    navigation(store, iterations, expected)
}

fn seed(path: &std::path::Path, count: usize) -> Result<(Store, Feed)> {
    let mut store = Store::open(path)?;
    let mut first = None;
    for source in 0..10 {
        let id =
            store.subscribe(&format!("https://bench.example/{source}/feed"))?;
        let feed = store
            .feeds()?
            .into_iter()
            .find(|feed| feed.id == id)
            .context("Missing feed")?;
        let start = source * count / 10;
        let end = (source + 1) * count / 10;
        for batch in (start..end).step_by(2000) {
            let articles = (batch..(batch + 2000).min(end))
                .map(incoming)
                .collect::<Vec<_>>();
            store.ingest(&feed, &format!("Journal {source}"), &articles)?;
        }
        if source == 0 {
            first = Some(feed);
        }
    }
    Ok((store, first.context("Missing first source")?))
}

fn incoming(index: usize) -> IncomingArticle {
    let title = match index % 3 {
        0 => "Kernel conntrack mechanisms",
        1 => "PostgreSQL transactions",
        _ => "Sourdough fermentation",
    };
    IncomingArticle {
        external_id: index.to_string(), title: format!("{title} {index}"),
        url: format!("https://bench.example/articles/{index}"),
        author: "Engineer".into(), published: 100 + index as i64,
        body: "Explicit ownership keeps behavior predictable. Measure before changing behavior.\n".repeat(20),
        body_origin: FeedOrigin::Feed,
    }
}

fn drain(store: &Store) -> Result<()> {
    loop {
        let update = store.refresh_recommendations()?;
        ensure!(
            update.processed <= 64 && update.total <= 100,
            "Resource limits changed"
        );
        if !update.remaining {
            return Ok(());
        }
    }
}

fn query() -> ArticleQuery {
    ArticleQuery {
        view: View::Recommended,
        text: String::new(),
        offset: 0,
        limit: 100,
        retained: None,
    }
}

fn cached(store: &Store, iterations: usize, expected: usize) -> Result<()> {
    super::measure(
        "cached For you count + metadata page + idle refresh",
        iterations * 5,
        || {
            let page = store.query_articles(&query())?;
            let eligible = expected;
            ensure!(
                page.total == eligible && page.articles.len() == eligible,
                "Recommendation cardinality changed: {}",
                page.total
            );
            ensure!(
                page.articles
                    .iter()
                    .all(|article| !article.title.contains("Sourdough")),
                "An unrelated document became a recommendation"
            );
            ensure!(
                store.refresh_recommendations()?.processed == 0,
                "Idle refresh rescanned text"
            );
            Ok(())
        },
    )
}

fn single_change(
    store: &mut Store,
    feed: &Feed,
    iterations: usize,
) -> Result<()> {
    let mut revision = 0;
    super::measure(
        "one changed publisher article + classification + publication",
        iterations,
        || {
            let mut changed = incoming(0);
            changed.author = format!("Engineer revision {revision}");
            revision += 1;
            store.ingest(feed, "Journal 0", &[changed])?;
            let update = store.refresh_recommendations()?;
            ensure!(
                update.processed == 1 && !update.remaining,
                "Change invalidated unrelated content"
            );
            Ok(())
        },
    )
}

fn navigation(store: Store, iterations: usize, _expected: usize) -> Result<()> {
    let mut app = App::new(store, false)?;
    app.set_view(View::Recommended)?;
    app.focus = lume::app::Focus::Articles;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 144, 44));
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    let mut latency = Vec::new();
    super::measure(
        "cached article down + up + actual frame",
        iterations * 5,
        || {
            let initial = app.current().context("Missing selection")?.id;
            let start = Instant::now();
            for code in [KeyCode::Down, KeyCode::Up] {
                app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
                terminal.draw(|frame| ui::draw(frame, &app))?;
            }
            latency.push(start.elapsed().as_secs_f64() * 1000.0);
            ensure!(
                app.current().map(|article| article.id) == Some(initial),
                "Navigation changed identity"
            );
            Ok(())
        },
    )?;
    latency.sort_by(f64::total_cmp);
    let at = |p: f64| latency[(latency.len() as f64 * p) as usize];
    println!(
        "recommendation navigation latency: p50={:.3} p95={:.3} p99={:.3} ms",
        at(0.5),
        at(0.95),
        at(0.99)
    );
    Ok(())
}

fn feedback(store: &Store) -> Result<()> {
    super::measure(
        "useful + clear feedback, cached evidence reweight",
        1,
        || {
            store.set_recommendation_feedback(
                1,
                Some(RecommendationFeedback::Useful),
            )?;
            drain(store)?;
            store.set_recommendation_feedback(1, None)?;
            drain(store)?;
            ensure!(
                store.recommendation_feedback(1)?.is_none(),
                "Feedback was not cleared"
            );
            Ok(())
        },
    )?;
    Ok(())
}
