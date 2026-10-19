//! Independent note search and interactive context latency at 10k/100k notes.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App},
    model::{FeedOrigin, IncomingArticle},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::{Duration, Instant};

const QUERY: &str = "body:quorum tag:consensus type:definition status:open";

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("notebook.sqlite3");
    let notes = if stress { 100_000 } else { 10_000 };
    let articles = notes / 100;
    let store = seed(&path, notes, articles)?;
    println!("notebook input: {notes} annotations, {articles} sources");
    search(&store, notes, iterations)?;
    context(&store, notes as i64, iterations)?;
    super::note_study::run(&store, notes as i64, iterations, stress)?;
    navigation(store, iterations)?;
    export(iterations, stress)
}

fn seed(
    path: &std::path::Path,
    notes: usize,
    articles: usize,
) -> Result<Store> {
    let mut store = Store::open(path)?;
    let id = store.subscribe("https://notes.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == id)
        .context("Missing feed")?;
    let rows = (0..articles).map(incoming).collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &rows)?;
    let mut connection = rusqlite::Connection::open(path)?;
    let transaction = connection.transaction()?;
    transaction.execute(
        include_str!("../../src/store/sql/test_setup_notebook_benchmark.sql"),
        (notes, articles),
    )?;
    transaction.execute_batch(include_str!(
        "../../src/store/sql/test_setup_notebook_benchmark_metadata.sql"
    ))?;
    transaction.commit()?;
    Ok(store)
}

fn incoming(index: usize) -> IncomingArticle {
    IncomingArticle {
        external_id: format!("source-{index}"),
        title: format!("Mechanisms {index}"),
        url: format!("https://notes.example/source/{index}"),
        author: "An engineer".into(),
        published: 2_000_000_000 + index as i64,
        body: "A durable mechanism needs one explicit owner.\n\nKeep the protocol precise and work bounded.".into(),
        body_origin: FeedOrigin::Feed,
    }
}

fn search(store: &Store, notes: usize, iterations: usize) -> Result<()> {
    super::measure(
        "note FTS + same-note filters + last page + snippets",
        iterations,
        || {
            let page =
                store.search_notes(QUERY, None, notes / 10 - 100, 100)?;
            ensure!(
                page.total == notes / 10 && page.notes.len() == 100,
                "Search changed cardinality"
            );
            ensure!(
                page.notes
                    .iter()
                    .all(|note| note.id % 10 == 0
                        && note.preview.contains('\u{1e}')),
                "Search lost phrase evidence"
            );
            Ok(())
        },
    )
}

fn context(store: &Store, note: i64, iterations: usize) -> Result<()> {
    super::measure(
        "owned note + source context + Markdown layout",
        iterations * 5,
        || {
            let detail = store.note_detail(note)?;
            let rendered = lume::reader::render(&detail.note.comment.body, 72);
            ensure!(
                detail.note.comment.id == note && !rendered.lines.is_empty(),
                "Context changed identity"
            );
            ensure!(
                detail.body.starts_with(&detail.note.comment.quote),
                "Context lost its source quote"
            );
            Ok(())
        },
    )
}

fn navigation(store: Store, iterations: usize) -> Result<()> {
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    command(&mut app, &format!("notes {QUERY}"))?;
    settle(&mut app)?;
    let article = app.current().context("Missing article")?.id;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    let mut timings = Vec::new();
    super::measure(
        "note key + ready context + actual frame",
        iterations * 5,
        || {
            let forward = timings.len() % 2 == 0;
            timings.push(advance_context(
                &mut app,
                &mut terminal,
                article,
                forward,
            )?);
            Ok(())
        },
    )?;
    timings.sort_by(f64::total_cmp);
    println!(
        "note context/frame latency: p50={:.3} p95={:.3} p99={:.3} ms",
        timings[timings.len() / 2],
        timings[timings.len() * 95 / 100],
        timings[timings.len() * 99 / 100]
    );
    Ok(())
}

fn advance_context(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    article: i64,
    forward: bool,
) -> Result<f64> {
    let previous = app
        .notebook
        .as_ref()
        .and_then(|book| book.current())
        .context("Missing note")?
        .id;
    let start = Instant::now();
    app.key(KeyEvent::new(
        KeyCode::Char(if forward { 'j' } else { 'k' }),
        KeyModifiers::NONE,
    ))?;
    settle(app)?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    let note = app
        .notebook
        .as_ref()
        .and_then(|book| book.current_context())
        .context("Missing ready context")?;
    ensure!(note.note.comment.id != previous, "Key did not move a note");
    ensure!(
        app.current().is_some_and(|item| item.id == article),
        "Note navigation changed article selection"
    );
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

fn export(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let count = if stress { 500 } else { 100 };
    let store = seed(&directory.path().join("exports.sqlite3"), count, count)?;
    let output = directory.path().join("markdown");
    super::measure("stable note files + citations + publication", 1, || {
        let report = lume::exchange::notes::notebook(&store, &output)?;
        ensure!(
            report.written == count && report.conflicts.is_empty(),
            "Export changed files"
        );
        Ok(())
    })?;
    super::measure(
        "incremental note files + exact-byte ownership",
        iterations,
        || {
            let report = lume::exchange::notes::notebook(&store, &output)?;
            ensure!(
                report.unchanged == count && report.written == 0,
                "Unchanged export rewrote files"
            );
            ensure!(
                std::fs::read_to_string(output.join("Note-1.md"))?
                    .contains("# Note 1"),
                "Export lost identity"
            );
            Ok(())
        },
    )
}

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    ensure!(
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?
            == Action::None,
        "Unexpected exit"
    );
    Ok(())
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.notebook_pending() {
        app.poll_note_workspace()?;
        ensure!(Instant::now() < deadline, "Notebook read timed out");
        std::thread::yield_now();
    }
    Ok(())
}
