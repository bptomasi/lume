//! Correctness-checked raster/vector preparation and cached scrolling frames.

#[path = "image_inspection.rs"]
mod inspection;

#[path = "sixel_images.rs"]
mod sixel;

use std::{
    io::Cursor,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use lume::{
    app::{App, Focus},
    media::{Decoder, ImageKey, cache},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let bytes = raster(stress)?;
    let svg = vector(stress);
    let key = ImageKey {
        url: "https://benchmark.example/diagram".into(),
        width: 80,
        height: 12,
        background: [23, 23, 26],
        detail: None,
    };
    let mut decoder = Decoder::default();
    decoder.render(svg.as_bytes(), &key)?;
    println!(
        "images: raster={} bytes, SVG={} bytes, stress={stress}",
        bytes.len(),
        svg.len()
    );
    super::measure(
        "cached raster source + halfblock encode",
        iterations,
        || verify_raster(&mut decoder, &bytes, &key),
    )?;
    super::measure(
        "cold raster decode + halfblock encode",
        iterations,
        || verify_raster(&mut Decoder::default(), &bytes, &key),
    )?;
    super::measure(
        "bounded SVG paths/text + halfblock encode",
        iterations,
        || verify_vector(&mut decoder, svg.as_bytes(), &key),
    )?;
    cached_frames(&bytes, iterations)?;
    inspection::run(&mut decoder, &bytes, svg.as_bytes(), &key, iterations)?;
    sixel::run(&mut decoder, svg.as_bytes(), &key, iterations)?;
    sixel::visible_bounds(iterations, stress)
}

fn verify_raster(
    decoder: &mut Decoder,
    bytes: &[u8],
    key: &ImageKey,
) -> Result<()> {
    let image = decoder.render(bytes, key)?;
    ensure!(
        image.cells.area.width <= 80 && image.cells.area.height <= 12,
        "Raster escaped its viewport"
    );
    ensure!(
        image
            .cells
            .content
            .iter()
            .any(|cell| cell.bg == Color::Rgb(255, 0, 0)),
        "Raster lost its independently known red region"
    );
    Ok(())
}

fn verify_vector(
    decoder: &mut Decoder,
    bytes: &[u8],
    key: &ImageKey,
) -> Result<()> {
    let image = decoder.render(bytes, key)?;
    ensure!(image.dimensions == (1200, 600), "SVG dimensions changed");
    ensure!(
        image
            .cells
            .content
            .iter()
            .any(|cell| cell.bg != Color::Rgb(23, 23, 26)),
        "SVG did not render any geometry"
    );
    Ok(())
}

pub(super) fn raster(stress: bool) -> Result<Vec<u8>> {
    let (width, height) = if stress { (2400, 1200) } else { (1200, 600) };
    let image = RgbImage::from_fn(width, height, |x, _| {
        if x < width / 2 {
            Rgb([255, 0, 0])
        } else {
            Rgb([0, 0, 255])
        }
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image).write_to(&mut bytes, ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

pub(super) fn vector(stress: bool) -> String {
    let count = if stress { 900 } else { 150 };
    let mut svg = String::from(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="600">"#,
    );
    for index in 0..count {
        let x = index % 30 * 40;
        let y = index / 30 * 20;
        svg.push_str(&format!(r##"<g><rect x="{x}" y="{y}" width="35" height="15" fill="#e1b36d"/><text x="{x}" y="{y}" font-size="8">service{index}</text></g>"##));
    }
    svg.push_str("</svg>");
    svg
}

fn cached_frames(bytes: &[u8], iterations: usize) -> Result<()> {
    let (directory, mut app) = image_app()?;
    let key = app.image_key(&app.images.slots[0]).context("Image key")?;
    let cache_dir = directory.path().join("images");
    cache::write(&cache_dir, &key.url, bytes)?;
    app.enable_images(cache_dir, true)?;
    settle(&mut app)?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::measure(
        "cached image scroll + two reader frames",
        iterations * 50,
        || image_frames(&mut app, &mut terminal),
    )
}

fn image_app() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://benchmark.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let body = format!(
        "Before image.\n\n![Diagram](https://benchmark.example/diagram)\n\n{}",
        "After image.\n\n".repeat(100)
    );
    let article = lume::model::IncomingArticle {
        external_id: "diagram".into(),
        title: "An architectural diagram".into(),
        url: "https://benchmark.example/article".into(),
        author: "Benchmark author".into(),
        published: 2_000_000_000,
        body,
        body_origin: lume::model::FeedOrigin::Feed,
    };
    store.ingest(&feed, "Image benchmark", &[article])?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    let column =
        ui::workspace_reader_content(Rect::new(0, 0, 144, 44), &app.workspace);
    app.resize_reader(column.width.into(), column.height.into());
    Ok((directory, app))
}

fn image_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
) -> Result<()> {
    let text = app.reader_text.len();
    app.scroll = app.images.slots[0].rows.start + 2;
    terminal.draw(|frame| ui::draw(frame, app))?;
    ensure!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.bg == Color::Rgb(255, 0, 0)),
        "Partially scrolled image disappeared"
    );
    app.scroll = 0;
    terminal.draw(|frame| ui::draw(frame, app))?;
    ensure!(app.reader_text.len() == text, "Drawing modified text");
    Ok(())
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        if !app.images_pending() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Image preparation timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}
