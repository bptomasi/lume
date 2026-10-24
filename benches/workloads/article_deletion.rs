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
    permanent(&mut app, &observer, iterations)?;
    app.end_session()?;
    batch_removal(iterations, if stress { 13_000 } else { 2000 })?;
    deletion_review(iterations.min(5))
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

fn permanent(app: &mut App, observer: &Store, iterations: usize) -> Result<()> {
    let mut elapsed = Duration::ZERO;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    for _ in 0..iterations {
        app.set_view(View::All)?;
        settle(app)?;
        let article = remove(app)?;
        settle(app)?;
        app.set_view(View::Trash)?;
        settle(app)?;
        let start = Instant::now();
        ensure!(remove(app)? == article, "Wrong permanent deletion target");
        while app.current().is_some() {
            tick(app)?;
            ensure!(start.elapsed() < Duration::from_secs(10), "Purge stalled");
            std::thread::sleep(Duration::from_millis(1));
        }
        terminal.draw(|frame| ui::draw_interactive(frame, app))?;
        elapsed += start.elapsed();
        ensure!(
            observer.article(article).is_err(),
            "Purge was not committed"
        );
        settle(app)?;
        ensure!(app.page_total == 0, "Purged article stayed in Trash");
    }
    print_sample("background Trash dd + committed frame", elapsed, iterations);
    Ok(())
}

fn batch_removal(iterations: usize, count: usize) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let base = directory.path().join("base.db");
    let mut store = Store::open(&base)?;
    seed(&mut store, count)?;
    let feed = store.feeds()?.remove(0).id;
    drop(store); // Last connection checkpoints WAL before cloning the fixture.
    for trash in [true, false] {
        let mut elapsed = Duration::ZERO;
        for _ in 0..iterations {
            let (_directory, store) = cloned_store(&base, trash)?;
            let start = Instant::now();
            if trash {
                ensure!(
                    store.clean_trash()? == count,
                    "Trash cleanup lost rows"
                );
            } else {
                store.unsubscribe(feed)?;
            }
            elapsed += start.elapsed();
            ensure!(
                store.library()?.articles.is_empty(),
                "Source retained unsaved rows"
            );
            ensure!(store.view_count(&View::Trash)? == 0, "Trash not empty");
            ensure!(!store.undo()?, "Cleanup created undo history");
        }
        let label = if trash {
            "bulk Trash cleanup"
        } else {
            "feed removal + unsaved articles"
        };
        print_sample(label, elapsed, iterations);
    }
    Ok(())
}

fn cloned_store(
    base: &std::path::Path,
    trash: bool,
) -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    std::fs::copy(base, &path)?;
    let store = Store::open(&path)?;
    if trash {
        rusqlite::Connection::open(&path)?.execute_batch(include_str!(
            "../../src/store/sql/deletion/bench_trash.sql"
        ))?;
    }
    Ok((directory, store))
}

fn deletion_review(iterations: usize) -> Result<()> {
    let (_directory, base) = cascade_fixture()?;
    for indexed in [false, true] {
        let mut elapsed = Duration::ZERO;
        for _ in 0..iterations {
            let (directory, store) = cloned_store(&base, true)?;
            if !indexed {
                rusqlite::Connection::open(
                    directory.path().join("library.sqlite3"),
                )?
                .execute_batch(include_str!(
                    "../../src/store/sql/deletion/bench_unindexed.sql"
                ))?;
            }
            let start = Instant::now();
            ensure!(store.clean_trash()? == 2000, "Cascade cleanup lost rows");
            elapsed += start.elapsed();
            ensure!(store.library()?.articles.is_empty(), "Parent survived");
            let remaining: usize = rusqlite::Connection::open(
                directory.path().join("library.sqlite3"))?.query_row(
                    include_str!("../../src/store/sql/deletion/bench_remaining_dependents.sql"),
                    [], |row| row.get(0),
                )?;
            ensure!(remaining == 0, "Dependent survived cascade");
        }
        print_sample(
            if indexed {
                "indexed note/card/export cascades"
            } else {
                "unindexed note/card/export cascades"
            },
            elapsed,
            iterations,
        );
    }
    bulk_view(iterations)
}

fn cascade_fixture() -> Result<(tempfile::TempDir, std::path::PathBuf)> {
    let directory = tempfile::tempdir()?;
    let base = directory.path().join("dependencies.db");
    let mut store = Store::open(&base)?;
    seed(&mut store, 2000)?;
    drop(store);
    let connection = rusqlite::Connection::open(&base)?;
    connection.execute_batch(include_str!(
        "../../src/store/sql/deletion/bench_dependencies.sql"
    ))?;
    drop(connection);
    Ok((directory, base))
}

fn bulk_view(iterations: usize) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("bulk.db"))?;
    seed(&mut store, 2000)?;
    let mut elapsed = Duration::ZERO;
    for _ in 0..iterations {
        let start = Instant::now();
        ensure!(store.trash_view(&View::All)? == 2000, "Bulk move lost rows");
        elapsed += start.elapsed();
        ensure!(
            store.view_count(&View::Trash)? == 2000,
            "Trash is incomplete"
        );
        ensure!(store.undo()?, "Bulk undo missing");
        ensure!(store.view_count(&View::All)? == 2000, "Bulk undo lost rows");
    }
    print_sample(
        "whole-library move with one undo snapshot",
        elapsed,
        iterations,
    );
    Ok(())
}
