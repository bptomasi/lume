//! Input-to-frame latency with a populated library, not an empty sidebar.

use std::time::Instant;

use anyhow::{Result, ensure};
use crossterm::event::KeyCode;
use lume::{
    app::{App, Focus},
    model::{
        FeedOrigin, IncomingArticle, RemoteFeedCandidate, SmartFilter, View,
    },
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let mut store = Store::open(&path)?;
    seed(&mut store, stress)?;
    let articles = store.view_count(&View::All)?;
    ensure!(articles == 13_000, "Representative article count changed");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.focus = Focus::Library;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    println!(
        "section input: {} feeds, {} smart lists, {} articles",
        app.library.feeds.len(),
        app.library.smart_lists.len(),
        articles
    );
    for group in ["FEEDS", "SMART LISTS", "SERVERS"] {
        measure(&mut app, &mut terminal, group, iterations)?;
    }
    app.end_session()
}

fn measure(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    group: &str,
    iterations: usize,
) -> Result<()> {
    app.set_view(View::Group(group.into()))?;
    let expected = section_children(app, group);
    ensure!(expected > 0, "Empty benchmark section");
    let mut samples = Vec::new();
    for _ in 0..iterations * 2 {
        let start = Instant::now();
        app.key(KeyCode::Enter.into())?;
        terminal.draw(|frame| ui::draw_interactive(frame, app))?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        let folded = app.workspace.collapsed_groups.iter().any(|s| s == group);
        ensure!(
            section_children(app, group) == if folded { 0 } else { expected },
            "Section lost destinations"
        );
        ensure!(
            app.view == View::Group(group.into()),
            "Header focus changed"
        );
        ensure!(
            !app.navigation_loading && !app.catalog_pending(),
            "A section toggle queued a database read"
        );
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "{group} Enter + frame: {} samples, p50={:.3} p95={:.3} max={:.3} ms",
        samples.len(),
        samples[samples.len() / 2],
        samples[(samples.len() * 95).div_ceil(100) - 1],
        samples[samples.len() - 1]
    );
    Ok(())
}

fn section_children(app: &App, group: &str) -> usize {
    app.navigation
        .iter()
        .filter(|item| {
            item.group == group && !matches!(item.view, View::Group(_))
        })
        .count()
}

fn seed(store: &mut Store, stress: bool) -> Result<()> {
    let feeds = if stress { 1000 } else { 100 };
    let per_feed = if stress { 13 } else { 130 };
    for index in 0..feeds {
        store.subscribe(&format!("https://publisher.example/{index}/feed"))?;
    }
    for feed in store.feeds()? {
        let articles = (0..per_feed)
            .map(|index| IncomingArticle {
                external_id: index.to_string(),
                title: format!("Systems rust {index}"),
                url: format!("https://publisher.example/{}/{index}", feed.id),
                body: "Rust systems scheduling and Kubernetes resources".into(),
                body_origin: FeedOrigin::Feed,
                author: String::new(),
                published: 1_800_000_000 + index,
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, &feed.title, &articles)?;
    }
    for index in 0..46 {
        store.save_smart_list(
            &format!("Topic {index}"),
            &SmartFilter {
                text: "systems OR rust".into(),
                ..Default::default()
            },
        )?;
    }
    seed_server(store, feeds)
}

fn seed_server(store: &mut Store, count: usize) -> Result<()> {
    let server = store.add_server("https://reader.example", "LUME_TOKEN")?;
    let candidates = (1..=count)
        .map(|index| RemoteFeedCandidate {
            remote_id: index as i64,
            title: format!("Remote {index}"),
            remote_url: format!("https://publisher.example/{index}"),
            local_url: format!("https://reader.example/v1/feeds/{index}"),
        })
        .collect::<Vec<_>>();
    store.sync_server_feeds(server, &candidates)
}
