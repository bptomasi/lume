//! Inspector pixels come from source bytes, with isolated modal interactions.

mod common;

use std::{
    io::Cursor,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind,
};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use lume::{
    app::{Action, App, Focus, InputBatch, Mode},
    media::{self, Decoder, ImageDetail, ImageKey, ImageProtocol},
    source, ui,
};
use ratatui::{
    Terminal, backend::TestBackend, buffer::Buffer, layout::Rect, style::Color,
};
use ratatui_image::Image;

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="64"><path fill="white" d="M0 0h128v64H0z"/><path fill="black" d="M64 0h1v64h-1z"/></svg>"##;

fn key(protocol: ImageProtocol, zoom: u8, pan: [u16; 2]) -> ImageKey {
    ImageKey {
        url: "https://journal.example/diagram".into(),
        width: 16,
        height: 8,
        background: [23, 23, 26],
        detail: Some(ImageDetail {
            protocol,
            zoom,
            pan,
            cell_pixels: (10, 20),
            slice: None,
        }),
    }
}

fn raster() -> Result<Vec<u8>> {
    let image = RgbImage::from_fn(128, 64, |x, _| {
        if x == 64 {
            Rgb([0, 0, 0])
        } else {
            Rgb([255, 255, 255])
        }
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(image).write_to(&mut bytes, ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    press(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    press(app, KeyCode::Enter)
}

fn reading() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body =
        "Before.\n\n![Diagram](/diagram)\n\nAfter.".into();
    store.ingest(&feed, &incoming.title, &incoming.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.reader_cursor = app.images.slots[0].caption;
    let key = app.image_key(&app.images.slots[0]).context("Inline key")?;
    let cache = directory.path().join("images");
    media::cache::write(&cache, &key.url, SVG)?;
    app.enable_images(cache, true)?;
    Ok((directory, app))
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        if !app.images_pending() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Image inspection timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn responsive_hidden_reader_does_not_admit_background_image_work() -> Result<()>
{
    let (_directory, mut app) = reading()?;
    app.workspace.fullscreen = false;
    app.focus = Focus::Library;
    app.resize_workspace(Rect::new(0, 0, 30, 15));
    app.poll_images();
    assert!(!app.images_pending());
    app.focus = Focus::Reader;
    app.resize_workspace(Rect::new(0, 0, 30, 15));
    settle(&mut app)?;
    let key = app
        .image_key(&app.images.slots[0])
        .context("Visible image")?;
    assert!(app.images.thumbnail(&key).is_some());
    Ok(())
}

#[derive(Debug, PartialEq)]
struct ReaderState {
    body: String,
    readable: String,
    scroll: usize,
    cursor: usize,
    selection: Option<usize>,
    workspace: String,
}

impl ReaderState {
    fn capture(app: &App) -> Result<Self> {
        Ok(Self {
            body: app.body.clone(),
            readable: app.reader_text.clone(),
            scroll: app.scroll,
            cursor: app.cursor_offset(),
            selection: app.selection,
            workspace: serde_json::to_string(&app.workspace)?,
        })
    }
}

#[test]
fn zoom_recovers_a_one_pixel_line_from_raster_and_vector_sources() -> Result<()>
{
    let mut decoder = Decoder::default();
    let key = key(ImageProtocol::Blocks, 8, [500, 500]);
    for bytes in [raster()?, SVG.to_vec()] {
        let image = decoder.render(&bytes, &key)?;
        assert_eq!(image.cells.area, Rect::new(0, 0, 16, 8));
        assert_eq!(image.dimensions, (128, 64));
        assert_eq!(image.cells[(8, 3)].fg, Color::Rgb(0, 0, 0));
        assert_eq!(image.cells[(8, 3)].bg, Color::Rgb(0, 0, 0));
        assert_eq!(image.cells[(7, 3)].fg, Color::Rgb(255, 255, 255));
        assert!(image.native.is_none());
    }
    Ok(())
}

#[test]
fn pan_reaches_source_edges_without_resizing_an_enlarged_canvas() -> Result<()>
{
    let mut decoder = Decoder::default();
    for pan in [[0, 0], [1000, 1000]] {
        let image = decoder.render(SVG, &key(ImageProtocol::Blocks, 8, pan))?;
        assert!(image.cells.content.iter().all(|cell| {
            cell.fg == Color::Rgb(255, 255, 255)
                && cell.bg == Color::Rgb(255, 255, 255)
        }));
    }
    let fractional = br#"<svg xmlns="http://www.w3.org/2000/svg" width="0.5" height="0.25"><path fill="red" d="M0 0h1v1H0z"/></svg>"#;
    let image =
        decoder.render(fractional, &key(ImageProtocol::Blocks, 1, [0, 0]))?;
    assert_eq!(image.dimensions, (1, 1));
    assert!(
        image
            .cells
            .content
            .iter()
            .any(|cell| cell.fg == Color::Rgb(255, 0, 0))
    );
    Ok(())
}

#[test]
fn native_encoders_receive_pixel_sized_images_and_remain_within_their_cells()
-> Result<()> {
    use ratatui::widgets::Widget;
    let mut decoder = Decoder::default();
    for protocol in [
        ImageProtocol::Kitty,
        ImageProtocol::Sixel,
        ImageProtocol::Iterm2,
    ] {
        let image = decoder.render(SVG, &key(protocol, 1, [0, 0]))?;
        let native = image.native.context("Native protocol was lost")?;
        assert_eq!(native.area(), Rect::new(0, 0, 16, 4));
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 10));
        Image::new(&native).render(Rect::new(2, 2, 16, 4), &mut buffer);
        let first = buffer[(2, 2)].symbol();
        assert!(first.contains('\u{1b}'));
        if protocol == ImageProtocol::Kitty {
            assert!(first.contains("s=160,v=80"), "Unexpected Kitty payload");
        }
        assert_eq!(buffer[(0, 0)].symbol(), " ");
        assert_eq!(buffer[(19, 9)].symbol(), " ");
    }
    Ok(())
}

#[test]
fn hostile_preparation_keys_are_rejected_before_allocating_viewports()
-> Result<()> {
    let mut keys = vec![
        key(ImageProtocol::Blocks, 0, [0, 0]),
        key(ImageProtocol::Blocks, 9, [0, 0]),
        key(ImageProtocol::Blocks, 1, [1001, 0]),
    ];
    let mut oversized = key(ImageProtocol::Kitty, 8, [0, 0]);
    oversized.width = 160;
    oversized.height = 48;
    oversized
        .detail
        .as_mut()
        .context("Detailed key")?
        .cell_pixels = (64, 128);
    keys.push(oversized);
    for key in keys {
        assert!(Decoder::default().render(SVG, &key).is_err());
    }
    Ok(())
}

#[test]
fn inspector_zoom_mouse_and_close_preserve_reader_selection_and_layout()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    app.selection = Some(app.cursor_offset());
    let before = ReaderState::capture(&app)?;
    press(&mut app, KeyCode::Enter)?;
    assert!(matches!(app.mode, Mode::Image(_)));
    press(&mut app, KeyCode::Char('+'))?;
    let mouse = MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 4,
        row: 4,
        modifiers: KeyModifiers::NONE,
    };
    let mut batch = InputBatch::new(vec![Event::Mouse(mouse); 3]);
    batch.dispatch_next(&mut app)?;
    assert!(
        matches!(&app.mode, Mode::Image(image) if image.zoom == 2 && image.pan == [0, 150])
    );
    settle(&mut app)?;
    let key = app.image_inspector_key().context("Inspector key")?;
    assert!(app.images.thumbnail(&key).is_some());
    press(&mut app, KeyCode::Esc)?;
    assert!(matches!(app.mode, Mode::Normal));
    assert_eq!(ReaderState::capture(&app)?, before);
    Ok(())
}

#[test]
fn explicit_inspection_loads_while_automatic_images_are_paused() -> Result<()> {
    let (_directory, mut app) = reading()?;
    command(&mut app, "images off")?;
    command(&mut app, "image")?;
    settle(&mut app)?;
    let key = app.image_inspector_key().context("Inspector key")?;
    assert!(app.images.thumbnail(&key).is_some());
    assert_eq!(press(&mut app, KeyCode::Char('o'))?, Action::Open(key.url));
    press(&mut app, KeyCode::Char('q'))?;
    app.focus = Focus::Articles;
    assert!(command(&mut app, "image").is_err());
    Ok(())
}

#[test]
fn inspector_retries_resize_keys_and_never_paints_native_pixels_into_reader()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    app.configure_image_terminal(ImageProtocol::Kitty, (10, 20));
    command(&mut app, "image")?;
    settle(&mut app)?;
    let old = app.image_inspector_key().context("Old key")?;
    assert!(
        app.images
            .thumbnail(&old)
            .context("Old image")?
            .native
            .is_some()
    );
    app.resize_workspace(Rect::new(0, 0, 80, 26));
    let new = app.image_inspector_key().context("Resized key")?;
    assert_ne!(old, new);
    settle(&mut app)?;
    let mut terminal = Terminal::new(TestBackend::new(80, 26))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.symbol().contains("_G"))
    );
    press(&mut app, KeyCode::Esc)?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    assert!(
        !terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.symbol().contains('\u{1b}'))
    );
    Ok(())
}

#[test]
fn fitting_an_image_avoids_redundant_pan_jobs_and_resets_the_crop() -> Result<()>
{
    let (_directory, mut app) = reading()?;
    command(&mut app, "image")?;
    settle(&mut app)?;
    let fitted = app.image_inspector_key().context("Fitted key")?;
    for _ in 0..32 {
        press(&mut app, KeyCode::Char('l'))?;
    }
    app.poll_images();
    assert_eq!(app.image_inspector_key(), Some(fitted.clone()));
    assert!(!app.images_pending());
    press(&mut app, KeyCode::Char('+'))?;
    press(&mut app, KeyCode::Char('l'))?;
    assert_ne!(app.image_inspector_key(), Some(fitted.clone()));
    press(&mut app, KeyCode::Char('-'))?;
    assert_eq!(app.image_inspector_key(), Some(fitted));
    Ok(())
}

#[test]
fn corrupt_offline_cache_keeps_the_decoder_reason_and_can_be_replaced()
-> Result<()> {
    let (directory, mut app) = reading()?;
    let key = app.image_key(&app.images.slots[0]).context("Inline key")?;
    let cache = directory.path().join("images");
    media::cache::write(&cache, &key.url, b"<html>Login required</html>")?;
    settle(&mut app)?;
    let error = app.images.error(&key).context("Offline failure")?;
    assert!(
        error.contains("Offline image cache")
            && error.contains("Supported images")
    );
    media::cache::write(&cache, &key.url, SVG)?;
    command(&mut app, "images retry")?;
    settle(&mut app)?;
    assert!(app.images.thumbnail(&key).is_some());
    Ok(())
}

#[test]
fn inspector_geometry_is_bounded_even_on_tiny_and_large_terminals() -> Result<()>
{
    let (_directory, mut app) = reading()?;
    command(&mut app, "image")?;
    for (width, height) in
        [(0, 0), (1, 1), (1, 20), (20, 1), (12, 5), (200, 80)]
    {
        let area = Rect::new(0, 0, width, height);
        app.resize_workspace(area);
        let content = ui::image_inspector_area(area);
        assert!(content.right() <= width && content.bottom() <= height);
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
    }
    Ok(())
}
