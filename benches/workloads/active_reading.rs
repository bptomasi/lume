//! Idle polls and changing viewports qualify bounded coverage without writes.
use anyhow::{Context, Result, ensure};
use crossterm::event::KeyCode;
use lume::{
    app::{App, Focus},
    model::View,
    source,
    store::Store,
};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let (_directory, mut app, observed) = application(stress)?;
    let target = app.current().context("Missing reading target")?.id;
    let mut now = Instant::now();
    app.poll_auto_read(now)?;
    measure_polls(&mut app, &mut now, iterations, false)?;
    measure_polls(&mut app, &mut now, iterations, true)?;
    ensure!(
        app.current().map(|article| article.id) == Some(target),
        "Selection moved"
    );
    ensure!(observed.article(target)?.unread, "Polling wrote read state");
    ensure!(observed.view_count(&View::Read)? == 0, "Read count changed");
    Ok(())
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App, Store)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("reading.sqlite3");
    let mut store = Store::open(&path)?;
    store.subscribe("https://example.test/feed.xml")?;
    let feed = store.feeds()?.into_iter().next().context("Missing feed")?;
    let mut parsed = source::parse(
        include_bytes!("../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    let copies = if stress { 2000 } else { 200 };
    parsed.articles[0].body =
        "A readable paragraph with words.\n\n".repeat(copies);
    store.ingest(&feed, &feed.title, &parsed.articles[..1])?;
    let observed = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    app.workspace.auto_read.min_active_seconds = 3600;
    app.focus = Focus::Articles;
    app.key(KeyCode::Enter.into())?;
    Ok((directory, app, observed))
}

fn measure_polls(
    app: &mut App,
    now: &mut Instant,
    iterations: usize,
    scroll: bool,
) -> Result<()> {
    let label = if scroll {
        "active reading: 1,000 changing viewport polls"
    } else {
        "active reading: 1,000 cached viewport polls"
    };
    super::measure(label, iterations, || {
        for _ in 0..1000 {
            if scroll {
                app.scroll = (app.scroll + app.reader_height.max(1))
                    % app.reader_lines.len().max(1);
            }
            *now += Duration::from_millis(1);
            ensure!(
                !app.poll_auto_read(*now)?,
                "Marked before active threshold"
            );
        }
        Ok(())
    })
}
