//! Read-state invalidation with many distinct, broad full-text smart lists.
use anyhow::{Context, Result, ensure};
use crossterm::event::KeyCode;
use lume::{
    app::{App, Focus},
    model::{IncomingArticle, SmartFilter, View},
    store::Store,
};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let mut store = Store::open(&path)?;
    seed(&mut store, if stress { 13_000 } else { 2000 })?;
    let observer = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    app.enable_background_navigation(&path)?;
    let count = iterations.min(8);
    let mut elapsed = Duration::ZERO;
    for index in 0..count {
        elapsed += sample(&mut app, &observer, index)?;
    }
    println!(
        "automatic read + distinct catalog counts: {count} iterations, {:.3} ms total, {:.3} ms/iteration",
        elapsed.as_secs_f64() * 1000.0,
        elapsed.as_secs_f64() * 1000.0 / count as f64
    );
    app.end_session()
}

fn sample(app: &mut App, observer: &Store, index: usize) -> Result<Duration> {
    settle(app)?;
    app.focus = Focus::Articles;
    app.key(KeyCode::Enter.into())?;
    app.workspace.auto_read.min_active_seconds = 1 + (index % 2) as u16;
    let id = app.current().context("Target")?.id;
    let started = Instant::now();
    for step in 0..=8 {
        app.poll_auto_read(started + Duration::from_millis(step * 250))?;
    }
    settle(app)?;
    let elapsed = started.elapsed();
    ensure!(!observer.article(id)?.unread, "Read was not committed");
    ensure!(
        app.navigation
            .iter()
            .find(|item| item.view == View::Read)
            .is_some_and(|item| item.count == 1),
        "Read count is stale"
    );
    app.key(KeyCode::Char('u').into())?;
    settle(app)?;
    ensure!(observer.article(id)?.unread, "Undo lost the source state");
    Ok(elapsed)
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.poll_note_workspace()?;
        app.poll_navigation()?;
        app.poll_reader();
        if !app.notes_pending()
            && !app.catalog_pending()
            && !app.article_preparing()
        {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Counts did not settle");
        std::thread::sleep(Duration::from_micros(100));
    }
}

fn seed(store: &mut Store, count: usize) -> Result<()> {
    store.subscribe("https://journal.test/feed")?;
    let feed = store.feeds()?.remove(0);
    for first in (0..count).step_by(1000) {
        let articles = (first..(first + 1000).min(count))
            .map(|id| IncomingArticle {
                external_id: format!("article:{id}"),
                title: format!("Systems article {id}"),
                url: format!("https://journal.test/{id}"),
                author: String::new(),
                published: 1_800_000_000 + id as i64,
                body: "Systems queues and resource scheduling.".into(),
                body_origin: lume::model::FeedOrigin::Feed,
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, &feed.title, &articles)?;
    }
    for id in 0..46 {
        store.save_smart_list(
            &format!("List {id:02}"),
            &SmartFilter {
                text: format!("systems AND NOT title:excluded{id:02}"),
                ..Default::default()
            },
        )?;
    }
    store.clear_history()
}
