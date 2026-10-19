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
    cached_frames(svg, iterations)?;
    cached_overlay_frames(svg, iterations)
}

fn native_encoders(
    decoder: &mut Decoder,
    bytes: &[u8],
    base: &ImageKey,
    iterations: usize,
) -> Result<()> {
    for protocol in [
        ImageProtocol::Kitty,
        ImageProtocol::Sixel,
        ImageProtocol::Iterm2,
        ImageProtocol::Overlay,
    ] {
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
    let native =
        !matches!(protocol, ImageProtocol::Blocks | ImageProtocol::Overlay);
    ensure!(
        image.native.is_some() == native,
        "Wrong presentation backend"
    );
    ensure!(
        image.dimensions.0 >= 1200 && image.dimensions.1 >= 600,
        "Inspector lost source dimensions"
    );
    if protocol == ImageProtocol::Overlay {
        let png = image.overlay.context("Overlay PNG missing")?;
        let size = image::ImageReader::open(png.path())?.into_dimensions()?;
        ensure!(size.0 > u32::from(key.width), "Overlay lost pixel detail");
    } else if !native {
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

fn cached_overlay_frames(bytes: &[u8], iterations: usize) -> Result<()> {
    let (directory, mut app) = super::image_app()?;
    app.configure_image_terminal(ImageProtocol::Overlay, (10, 20));
    let key = app.image_key(&app.images.slots[0]).context("Overlay key")?;
    let cache_dir = directory.path().join("images");
    cache::write(&cache_dir, &key.url, bytes)?;
    app.enable_images(cache_dir, true)?;
    super::settle(&mut app)?;
    app.scroll = app.images.slots[0].rows.start + 2;
    super::settle(&mut app)?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    super::super::measure(
        "cached overlay projection + two reader frames",
        iterations * 50,
        || {
            for scroll in [0, app.images.slots[0].rows.start + 2] {
                app.scroll = scroll;
                app.poll_images();
                ensure!(!app.images_pending(), "Cached crop started new work");
                let images = app.overlay_placements();
                ensure!(
                    images.len() == 1,
                    "Visible image projection disappeared"
                );
                ensure!(
                    images[0].image.bytes() > 0 && images[0].area.height > 0,
                    "Projection lost its prepared pixels"
                );
                terminal.draw(|frame| ui::draw(frame, &app))?;
            }
            Ok(())
        },
    )?;
    scrolling_overlay_frames(&mut app, &mut terminal, iterations)
}

fn scrolling_overlay_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let first = app.images.slots[0].rows.start;
    let original = app.overlay_placements()[0].image.clone();
    super::super::measure(
        "cached bidirectional scroll + 24 clipped frames",
        iterations * 5,
        || {
            for offset in (0..12).chain((0..12).rev()) {
                app.scroll = first + offset;
                app.poll_images();
                ensure!(
                    !app.images_pending(),
                    "Scroll admitted image decoding"
                );
                let images = app.overlay_placements();
                ensure!(images.len() == 1, "Scroll lost its visible image");
                ensure!(
                    images[0].image.path() == original.path(),
                    "Scroll replaced its fitted source"
                );
                terminal.draw(|frame| ui::draw(frame, app))?;
            }
            Ok(())
        },
    )
}
