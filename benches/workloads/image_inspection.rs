//! Source zoom, all native encoders and cached modal input are checked separately.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode},
    media::{Decoder, ImageDetail, ImageKey, ImageProtocol, cache},
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub(super) fn run(
    decoder: &mut Decoder,
    raster: &[u8],
    svg: &[u8],
    base: &ImageKey,
    iterations: usize,
) -> Result<()> {
    let mut key = base.clone();
    key.width = 120;
    key.height = 36;
    key.detail = Some(ImageDetail {
        zoom: 4,
        pan: [500, 500],
        protocol: ImageProtocol::Blocks,
        cell_pixels: (10, 20),
        slice: None,
    });
    for (name, bytes) in [
        ("source raster crop + detailed blocks", raster),
        ("source SVG zoom + detailed blocks", svg),
    ] {
        super::super::measure(name, iterations, || {
            verify(decoder, bytes, &key)
        })?;
    }
    native_encoders(decoder, svg, &key, iterations)?;
    cached_frames(svg, iterations)
}

fn native_encoders(
    decoder: &mut Decoder,
    bytes: &[u8],
    base: &ImageKey,
    iterations: usize,
) -> Result<()> {
    for protocol in [ImageProtocol::Sixel, ImageProtocol::Iterm2] {
        let mut key = base.clone();
        let detail = key.detail.as_mut().context("Inspector settings")?;
        detail.protocol = protocol;
        detail.zoom = 1;
        super::super::measure(
            &format!("source SVG + {protocol:?} pixel encoding"),
            iterations,
            || verify(decoder, bytes, &key),
        )?;
    }
    Ok(())
}

fn verify(decoder: &mut Decoder, bytes: &[u8], key: &ImageKey) -> Result<()> {
    let image = decoder.render(bytes, key)?;
    ensure!(
        image.cells.area.width <= key.width
            && image.cells.area.height <= key.height,
        "Inspector escaped its cells"
    );
    let protocol = key
        .detail
        .as_ref()
        .map_or(ImageProtocol::Blocks, |detail| detail.protocol);
    let native = protocol != ImageProtocol::Blocks;
    ensure!(
        image.native.is_some() == native,
        "Wrong presentation backend"
    );
    ensure!(
        image.dimensions.0 >= 1200 && image.dimensions.1 >= 600,
        "Inspector lost source dimensions"
    );
    if !native {
        ensure!(
            image.cells.content.iter().any(|cell| cell.symbol() == "▀"),
            "Detailed source pixels disappeared"
        );
    }
    Ok(())
}

fn cached_frames(bytes: &[u8], iterations: usize) -> Result<()> {
    let (directory, mut app) = super::image_app()?;
    let key = app.image_key(&app.images.slots[0]).context("Inline key")?;
    let cache_dir = directory.path().join("images");
    cache::write(&cache_dir, &key.url, bytes)?;
    app.enable_images(cache_dir, true)?;
    app.reader_cursor = app.images.slots[0].caption;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    press(&mut app, '+')?;
    super::settle(&mut app)?;
    press(&mut app, 'l')?;
    super::settle(&mut app)?;
    press(&mut app, 'h')?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::super::measure(
        "cached inspector pan + two frames",
        iterations * 50,
        || inspect_frames(&mut app, &mut terminal),
    )
}

fn press(app: &mut App, character: char) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    Ok(())
}

fn inspect_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
) -> Result<()> {
    for character in ['l', 'h'] {
        press(app, character)?;
        app.poll_images();
        ensure!(!app.images_pending(), "Cached pan started preparation");
        terminal.draw(|frame| ui::draw(frame, app))?;
        ensure!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.symbol() == "▀"),
            "Inspector pixels disappeared"
        );
    }
    ensure!(
        matches!(app.mode, Mode::Image(_)),
        "Pan escaped the inspector"
    );
    Ok(())
}
