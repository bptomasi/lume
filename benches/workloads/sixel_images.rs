//! Cell-height Sixel strips reuse source pixels through bidirectional scrolling.

use anyhow::{Context, Result, ensure};
use lume::{
    app::App,
    media::{Decoder, ImageDetail, ImageKey, ImageProtocol, ImageSlice, cache},
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) fn run(
    decoder: &mut Decoder,
    bytes: &[u8],
    base: &ImageKey,
    iterations: usize,
) -> Result<()> {
    let mut key = base.clone();
    key.detail = Some(ImageDetail {
        zoom: 1,
        pan: [0, 0],
        protocol: ImageProtocol::Sixel,
        cell_pixels: (10, 20),
        slice: Some(ImageSlice {
            first_row: 0,
            rows: 12,
        }),
    });
    super::super::measure(
        "source SVG + inline Sixel strips",
        iterations,
        || {
            let thumbnail = decoder.render(bytes, &key)?;
            let rows = thumbnail.rows.context("Sixel rows missing")?;
            ensure!(rows.bytes() > 0, "Pixel strips have no encoded data");
            ensure!(thumbnail.native.is_none(), "Expected inline row encoding");
            Ok(())
        },
    )?;
    cached_frames(bytes, iterations)
}

fn cached_frames(bytes: &[u8], iterations: usize) -> Result<()> {
    let (directory, mut app) = super::image_app()?;
    app.configure_image_terminal(ImageProtocol::Sixel, (10, 20));
    let key = app
        .image_key(&app.images.slots[0])
        .context("Inline Sixel key")?;
    let cache_dir = directory.path().join("images");
    cache::write(&cache_dir, &key.url, bytes)?;
    app.enable_images(cache_dir, true)?;
    super::settle(&mut app)?;
    let area = app
        .images
        .thumbnail(&key)
        .context("Ready pixels")?
        .cells
        .area;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::super::measure(
        "cached Sixel bidirectional scroll + clipped frames",
        iterations * 5,
        || scroll_frames(&mut app, &mut terminal, area.width, area.height),
    )
}

fn scroll_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    width: u16,
    height: u16,
) -> Result<()> {
    let first = app.images.slots[0].rows.start;
    let cleared_strip = format!("\x1b[{width}X\x1bP");
    for offset in (0..height).chain((0..height).rev()) {
        app.scroll = first + usize::from(offset);
        app.poll_images();
        ensure!(!app.images_pending(), "Scroll admitted image preparation");
        terminal.draw(|frame| ui::draw(frame, app))?;
        ensure!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.symbol().starts_with(&cleared_strip)),
            "Clipped Sixel image disappeared"
        );
    }
    Ok(())
}

pub(super) fn visible_bounds(iterations: usize, stress: bool) -> Result<()> {
    use lume::{app::ReaderImages, reader::images::ImageSlot};
    use std::hint::black_box;

    let count = if stress { 50_000 } else { 10_000 };
    let mut images = ReaderImages::default();
    images.slots = (0..count)
        .map(|index| ImageSlot {
            formula: None,
            url: format!("https://benchmark.example/{index}.svg"),
            alt: String::new(),
            caption: index * 20,
            rows: index * 20 + 1..index * 20 + 13,
            source: index..index + 1,
        })
        .collect();
    super::super::measure(
        "visible image bounds across 128 long-article positions",
        iterations * 50,
        || {
            for step in 0..128 {
                let index = count / 2 + step;
                let visible =
                    black_box(&images).visible_slots(index * 20 + 2, 30);
                ensure!(visible.len() == 2, "Visible image count changed");
                ensure!(
                    visible[0].source.start == index,
                    "Wrong clipped image"
                );
                ensure!(
                    visible[1].source.start == index + 1,
                    "Wrong next image"
                );
            }
            Ok(())
        },
    )
}
