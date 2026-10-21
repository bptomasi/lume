//! File snapshots and asynchronous navigation use real representative libraries.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use lume::{
    app::App,
    model::{FeedOrigin, IncomingArticle, SmartFilter, View},
    store::Store,
};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = fixture(&database, stress)?;
    let expected = store.view_count(&View::All)?;
    println!(
        "recovery/navigation: {expected} articles, {} sources, {} queries",
        store.feeds()?.len(),
        store.smart_lists()?.len()
    );
    let mut cycle = 0;
    super::measure(
        "online snapshot + isolated validated restore",
        iterations,
        || {
            cycle += 1;
            let backup =
                directory.path().join(format!("snapshot-{cycle}.sqlite3"));
            let restored = directory.path().join(format!("restored-{cycle}"));
            store.backup_to(&backup)?;
            Store::restore_snapshot(&backup, &restored)?;
            let recovered = Store::open(&restored.join("library.sqlite3"))?;
            ensure!(
                recovered.view_count(&View::All)? == expected,
                "Snapshot lost articles"
            );
            ensure!(
                recovered.catalog()?.smart_lists.len()
                    == store.smart_lists()?.len(),
                "Snapshot lost query definitions"
            );
            Ok(())
        },
    )?;
    startup(&database, iterations, expected)
}

fn startup(
    database: &std::path::Path,
    iterations: usize,
    expected: usize,
) -> Result<()> {
    let mut deliveries = Vec::new();
    let mut admissions = Vec::new();
    super::measure(
        "startup metadata + workers + restored reader delivery",
        iterations,
        || {
            let started = Instant::now();
            let mut app = App::open_interactive(Store::open(database)?, false)?;
            ensure!(app.body.is_empty(), "Startup eagerly loaded a body");
            app.enable_background_navigation(database)?;
            app.load_workspace();
            deliveries.push(settle(&mut app, started)?.as_secs_f64() * 1000.0);
            ensure!(
                app.page_total == expected,
                "Wrong startup destination count"
            );
            let id = app.current().context("Startup article")?.id;
            let started = Instant::now();
            app.jump_article(id)?;
            admissions.push(started.elapsed().as_secs_f64() * 1_000_000.0);
            ensure!(app.navigation_loading, "Jump did not defer work");
            settle(&mut app, started)?;
            ensure!(
                app.reader_identity()
                    .is_some_and(|identity| identity.article == id),
                "Jump displayed a different article"
            );
            app.end_session()
        },
    )?;
    println!(
        "startup first prepared reader: {:.3} ms; jump admission: {:.3} µs (means)",
        deliveries.iter().sum::<f64>() / deliveries.len() as f64,
        admissions.iter().sum::<f64>() / admissions.len() as f64
    );
    Ok(())
}

fn settle(app: &mut App, started: Instant) -> Result<Duration> {
    let deadline = started + Duration::from_secs(10);
    let mut first = None;
    while app.navigation_loading
        || app.catalog_pending()
        || app.reader_pending()
    {
        ensure!(Instant::now() < deadline, "Navigation delivery timed out");
        app.poll_reader();
        app.poll_navigation()?;
        app.poll_note_tasks()?;
        if first.is_none()
            && !app.navigation_loading
            && !app.article_preparing()
        {
            first = Some(started.elapsed());
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    Ok(first.unwrap_or_else(|| started.elapsed()))
}

fn fixture(path: &std::path::Path, stress: bool) -> Result<Store> {
    let mut store = Store::open(path)?;
    let sources = if stress { 40 } else { 4 };
    let per_source = if stress { 100 } else { 25 };
    for index in 0..sources {
        let id = store
            .subscribe(&format!("https://journal.example/{index}/feed"))?;
        let feed = store
            .feeds()?
            .into_iter()
            .find(|feed| feed.id == id)
            .context("Fixture feed")?;
        let articles = (0..per_source).map(|item| IncomingArticle {
            external_id: item.to_string(), title: format!("Consensus and transaction {item}"),
            url: format!("https://journal.example/{index}/{item}"), author: "Fixture".into(),
            published: 1_700_000_000 + item, body_origin: FeedOrigin::Feed,
            body: "# Consensus\n\nA transaction preserves the admitted intention.\n".repeat(25),
        }).collect::<Vec<_>>();
        store.ingest(&feed, "Fixture", &articles)?;
    }
    for index in 0..if stress { 200 } else { 12 } {
        store.save_smart_list(
            &format!("Query {index}"),
            &SmartFilter {
                text: "transaction OR consensus".into(),
                ..Default::default()
            },
        )?;
    }
    let mut workspace = store.workspace()?;
    workspace.view = View::All;
    store.save_workspace(&workspace)?;
    Ok(store)
}
