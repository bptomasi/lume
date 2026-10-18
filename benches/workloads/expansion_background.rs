//! Cached reader interaction while a real SQLite publisher writer is active.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use crossterm::event::KeyCode;
use lume::{
    app::App,
    model::{FeedOrigin, IncomingArticle},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) fn run(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    path: &Path,
    samples: usize,
    stress: bool,
) -> Result<()> {
    let stopped = Arc::new(AtomicBool::new(false));
    let cancellation = Arc::clone(&stopped);
    let (started, receiver) = mpsc::sync_channel(1);
    let path = path.to_owned();
    let worker = thread::Builder::new()
        .name("lume-benchmark-writer".into())
        .spawn(move || write_updates(path, cancellation, started, stress))?;
    let ready = receiver
        .recv_timeout(Duration::from_secs(5))
        .context("Publisher writer did not start");
    let measured = ready.and_then(|()| measure_reader(app, terminal, samples));
    stopped.store(true, Ordering::Relaxed);
    let writes = worker
        .join()
        .map_err(|_| anyhow::anyhow!("Publisher writer panicked"))??;
    measured?;
    ensure!(writes > 0, "No concurrent publisher write was exercised");
    println!("concurrent publisher batches committed={writes}");
    Ok(())
}

fn write_updates(
    path: PathBuf,
    stopped: Arc<AtomicBool>,
    started: mpsc::SyncSender<()>,
    stress: bool,
) -> Result<usize> {
    let mut store = Store::open(&path)?;
    let feed = store.feeds()?.remove(0);
    let count = if stress { 500 } else { 100 };
    let mut entries = (0..count)
        .map(|index| IncomingArticle {
            external_id: index.to_string(),
            title: format!("Publisher update {index}"),
            url: format!("https://benchmark.example/article/{index}"),
            author: "Original benchmark writer".into(),
            published: 100 + index as i64,
            body: String::new(),
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    let mut writes = 0;
    for revision in 0..1000 {
        for entry in &mut entries {
            entry.body =
                format!("Bounded publisher update revision {revision}.\n")
                    .repeat(20);
        }
        store.ingest(&feed, &feed.title, &entries)?;
        writes += 1;
        if revision == 0 {
            started.send(())?;
        }
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        thread::sleep(Duration::from_millis(2));
    }
    Ok(writes)
}

fn measure_reader(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    samples: usize,
) -> Result<()> {
    super::latency(
        "reader input/frame during publisher writes",
        samples,
        |index| {
            let before = app.reader_cursor;
            super::press(
                app,
                if index % 2 == 0 {
                    KeyCode::Down
                } else {
                    KeyCode::Up
                },
            )?;
            terminal.draw(|frame| ui::draw(frame, app))?;
            ensure!(app.reader_cursor != before, "Publisher writes lost input");
            ensure!(
                app.current().is_some_and(
                    |article| article.title == "Measured reader document"
                ),
                "Publisher write retargeted the reader"
            );
            Ok(())
        },
    )
}
