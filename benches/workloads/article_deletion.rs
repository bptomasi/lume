//! Durable deletion-to-frame costs include populated smart-list counts and undo.

use anyhow::{Context, Result, ensure};
use crossterm::event::KeyCode;
use lume::{
    app::{App, Focus},
    model::{IncomingArticle, SmartFilter, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let mut store = Store::open(&path)?;
    seed(&mut store, if stress { 13_000 } else { 2000 })?;
    let observer = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    synchronous(&mut app, &observer, iterations)?;
    app.enable_background_navigation(&path)?;
    asynchronous(&mut app, &observer, iterations)?;
    app.end_session()
}

fn seed(store: &mut Store, count: usize) -> Result<()> {
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    for start in (0..count).step_by(1000) {
        let entries = (start..(start + 1000).min(count))
            .map(|id| IncomingArticle {
                external_id: format!("article:{id}"),
                title: format!("Systems article {id}"),
                url: format!("https://journal.example/{id}"),
                author: "Author".into(),
                published: 1_800_000_000 + id as i64,
                body: "Systems queues and efficient software. ".repeat(8),
                body_origin: lume::model::FeedOrigin::Feed,
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, &feed.title, &entries)?;
    }
    for id in 0..46 {
        store.save_smart_list(
            &format!("Reading list {id:02}"),
            &SmartFilter {
                text: "systems".into(),
                ..Default::default()
            },
        )?;
    }
    store.clear_history()
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        tick(app)?;
        if !app.notes_pending()
            && !app.catalog_pending()
            && !app.article_preparing()
        {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Deletion did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn tick(app: &mut App) -> Result<()> {
    app.poll_note_workspace()?;
    app.poll_navigation()?;
    app.poll_reader();
    Ok(())
}

fn remove(app: &mut App) -> Result<i64> {
    app.focus = Focus::Articles;
    let article = app.current().context("Missing deletion target")?.id;
    app.key(KeyCode::Char('d').into())?;
    app.key(KeyCode::Char('d').into())?;
    Ok(article)
}

fn synchronous(
    app: &mut App,
    observer: &Store,
    iterations: usize,
) -> Result<()> {
    let mut elapsed = Duration::ZERO;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    for _ in 0..iterations {
        let start = Instant::now();
        let article = remove(app)?;
        terminal.draw(|frame| ui::draw_interactive(frame, app))?;
        elapsed += start.elapsed();
        ensure!(observer.is_deleted(article)?, "Deletion was not committed");
        app.key(KeyCode::Char('u').into())?;
        ensure!(!observer.is_deleted(article)?, "Undo failed");
    }
    print_sample("synchronous dd + committed frame", elapsed, iterations);
    Ok(())
}

fn asynchronous(
    app: &mut App,
    observer: &Store,
    iterations: usize,
) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    let mut visible = Duration::ZERO;
    let mut complete = Duration::ZERO;
    for _ in 0..iterations {
        settle(app)?;
        let count = app.page_total;
        let start = Instant::now();
        let article = remove(app)?;
        while app.library.articles.iter().any(|item| item.id == article) {
            tick(app)?;
            ensure!(
                start.elapsed() < Duration::from_secs(10),
                "Deletion stayed visible"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        terminal.draw(|frame| ui::draw_interactive(frame, app))?;
        visible += start.elapsed();
        ensure!(
            observer.is_deleted(article)?,
            "Visible removal preceded commit"
        );
        settle(app)?;
        complete += start.elapsed();
        ensure!(app.page_total == count - 1, "Page count is stale");
        ensure!(
            observer.view_count(&View::Trash)? == 1,
            "Wrong deletion count"
        );
        app.key(KeyCode::Char('u').into())?;
        settle(app)?;
        ensure!(!observer.is_deleted(article)?, "Ordered undo failed");
    }
    print_sample("background dd + first committed frame", visible, iterations);
    print_sample(
        "background dd + settled page and counts",
        complete,
        iterations,
    );
    Ok(())
}

fn print_sample(name: &str, elapsed: Duration, iterations: usize) {
    let millis = elapsed.as_secs_f64() * 1000.0;
    println!(
        "{name}: {iterations} iterations, {millis:.3} ms total, {:.3} ms/iteration",
        millis / iterations as f64
    );
}
