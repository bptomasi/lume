//! Export a real offline image Reader frame without contacting a publisher.

mod support;

use std::{
    path::PathBuf,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    media,
    model::IncomingArticle,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const ADDRESS: &str = "https://example.org/request-path.svg";
const DIAGRAM: &[u8] =
    include_bytes!("../tests/fixtures/images/request-path.svg");

fn main() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("/tmp/lume-images.json"), PathBuf::from);
    let directory = tempfile::tempdir()?;
    let store = seed(&directory.path().join("preview.sqlite3"))?;
    let mut app = App::new(store, true)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    let column =
        ui::workspace_reader_content(Rect::new(0, 0, 144, 44), &app.workspace);
    app.resize_reader(column.width.into(), column.height.into());
    let cache = directory.path().join("images");
    media::cache::write(&cache, ADDRESS, DIAGRAM)?;
    app.enable_images(cache, true)?;
    if std::env::args().any(|argument| argument == "--inspect") {
        app.reader_cursor = app.images.slots[0].caption;
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
        app.key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE))?;
    }
    settle(&mut app)?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    support::export(terminal.backend().buffer(), &output)?;
    println!("{}", output.display());
    Ok(())
}

fn seed(path: &std::path::Path) -> Result<Store> {
    let mut store = Store::open(path)?;
    store.subscribe("https://example.org/feed")?;
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Architecture Journal",
        &[IncomingArticle {
            external_id: "request-path".into(),
            title: "Following a request through the system".into(),
            url: "https://example.org/article".into(),
            author: "Lume example".into(),
            published: 1_800_000_000,
            body: include_str!("../tests/fixtures/images/request-path.md")
                .into(),
            body_origin: lume::model::FeedOrigin::Feed,
        }],
    )?;
    Ok(store)
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        if !app.images_pending() {
            let key = app
                .image_inspector_key()
                .or_else(|| app.image_key(&app.images.slots[0]))
                .context("Image key")?;
            ensure!(
                app.images.thumbnail(&key).is_some(),
                "Image preview failed: {:?}",
                app.images.error(&key)
            );
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Image preview timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
