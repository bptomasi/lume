//! Scroll frames and empty-cache cleanup keep independently checked state.

use anyhow::{Context, Result, ensure};
use crossterm::event::KeyCode;
use lume::{
    app::{App, Focus},
    model::IncomingArticle,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::sync::atomic::AtomicBool;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let count = if stress { 2000 } else { 200 };
    let body = (0..16)
        .map(|id| format!("![image](https://images.example/{id}.png)\n\n"))
        .collect::<String>();
    let entries = (0..count)
        .map(|id| IncomingArticle {
            external_id: format!("entry:{id}"),
            title: format!(
                "Article {id} {}",
                "Unicode systems 界 and queues ".repeat(if stress {
                    30
                } else {
                    8
                })
            ),
            url: format!("https://journal.example/{id}"),
            author: "Writer".into(),
            published: 1_800_000_000,
            body: body.clone(),
            body_origin: lume::model::FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &entries)?;
    empty_cleanup(&store, directory.path(), iterations)?;
    scrolling(store, iterations)
}

fn empty_cleanup(
    store: &Store,
    root: &std::path::Path,
    iterations: usize,
) -> Result<()> {
    let images = root.join("images");
    super::measure("empty image-cache cleanup", iterations, || {
        let report = lume::storage::clean(
            store,
            &images,
            lume::storage::Cleanup::Images,
            &AtomicBool::new(false),
        )?;
        ensure!(report.contains("Removed 0 orphaned"), "Wrong file count");
        ensure!(!images.exists(), "Cleanup created the missing cache");
        Ok(())
    })
}

fn scrolling(store: Store, iterations: usize) -> Result<()> {
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let source = app.body.clone();
    let article = app.current().context("Selected article")?.id;
    super::measure(
        "reader motion pair + two full pane frames",
        iterations * 30,
        || {
            let cursor = app.reader_cursor;
            app.key(KeyCode::Char('j').into())?;
            ensure!(app.reader_cursor == cursor + 1, "Down motion failed");
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.key(KeyCode::Char('k').into())?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            ensure!(app.reader_cursor == cursor, "Reverse motion failed");
            ensure!(app.body == source, "Scroll changed publisher content");
            ensure!(
                app.current().map(|item| item.id) == Some(article),
                "Changed article"
            );
            Ok(())
        },
    )
}
