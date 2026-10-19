//! Native Foot strips preserve pixels and reader identity through clipped scroll.

mod common;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use lume::{
    app::{App, Focus, Mode},
    media::{self, Decoder, ImageDetail, ImageKey, ImageProtocol, ImageSlice},
    source, ui,
};
use ratatui::{Terminal, backend::TestBackend, buffer::Buffer, layout::Rect};

const SVG: &[u8] = include_bytes!("fixtures/images/request-path.svg");

fn key() -> ImageKey {
    ImageKey {
        url: "https://example.org/diagram.svg".into(),
        width: 80,
        height: 12,
        background: [23, 23, 26],
        detail: Some(ImageDetail {
            zoom: 1,
            pan: [0, 0],
            protocol: ImageProtocol::Sixel,
            cell_pixels: (10, 20),
            slice: Some(ImageSlice {
                first_row: 0,
                rows: 12,
            }),
        }),
    }
}

#[test]
fn clipped_strips_keep_exact_source_rows_and_cell_height() -> Result<()> {
    let image = Decoder::default().render(SVG, &key())?;
    let rows = image.rows.context("Native Sixel strips")?;
    let mut complete = Buffer::empty(Rect::new(0, 0, 100, 20));
    rows.render(Rect::new(3, 2, 80, 12), 0, &mut complete);
    let mut clipped = Buffer::empty(complete.area);

    rows.render(Rect::new(3, 2, 80, 4), 2, &mut clipped);

    for row in 0..4 {
        let actual = clipped[(3, 2 + row)].symbol();
        assert_eq!(actual, complete[(3, 4 + row)].symbol());
        assert!(actual.starts_with("\x1bP"));
        assert!(actual.contains("\"1;1;800;20"));
        assert!(clipped[(4, 2 + row)].skip);
    }
    assert_eq!(clipped[(2, 2)].symbol(), " ");
    assert_eq!(clipped[(3, 6)].symbol(), " ");
    assert!(rows.bytes() > 80 * 12);
    Ok(())
}

#[test]
fn empty_narrow_and_out_of_bounds_targets_do_not_paint() -> Result<()> {
    let image = Decoder::default().render(SVG, &key())?;
    let rows = image.rows.context("Native Sixel strips")?;
    let mut cells = Buffer::empty(Rect::new(0, 0, 100, 20));
    for area in [
        Rect::new(0, 0, 0, 0),
        Rect::new(0, 0, 79, 4),
        Rect::new(30, 0, 80, 4),
    ] {
        rows.render(area, 0, &mut cells);
    }
    rows.render(Rect::new(0, 0, 80, 4), 12, &mut cells);
    assert!(cells.content.iter().all(|cell| cell.symbol() == " "));
    Ok(())
}

#[test]
fn partial_final_row_is_not_stretched_to_a_whole_terminal_cell() -> Result<()> {
    let mut narrow = key();
    narrow.width = 57;
    let image = Decoder::default().render(SVG, &narrow)?;
    let rows = image.rows.context("Native Sixel strips")?;
    assert_eq!(image.cells.area.height, 9);
    let mut cells = Buffer::empty(Rect::new(0, 0, 80, 12));

    rows.render(Rect::new(0, 0, 57, 1), 8, &mut cells);

    assert!(cells[(0, 0)].symbol().contains("\"1;1;570;11"));
    assert_eq!(cells[(0, 1)].symbol(), " ");
    Ok(())
}

fn reading() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body = format!(
        "![Diagram](https://example.org/diagram.svg)\n\n{}",
        "After image.\n\n".repeat(50)
    );
    store.ingest(&feed, &incoming.title, &incoming.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.configure_image_terminal(ImageProtocol::Sixel, (10, 20));
    app.resize_workspace(Rect::new(0, 0, 100, 36));
    let cache = directory.path().join("images");
    media::cache::write(&cache, &key().url, SVG)?;
    app.enable_images(cache, true)?;
    settle(&mut app)?;
    Ok((directory, app))
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        if !app.images_pending() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "Sixel image preparation timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn native_rows(cells: &Buffer) -> usize {
    cells
        .content
        .iter()
        .filter(|cell| cell.symbol().starts_with("\x1bP"))
        .count()
}

#[test]
fn both_scroll_directions_keep_the_same_prepared_image_and_source() -> Result<()>
{
    let (_directory, mut app) = reading()?;
    let text = app.reader_text.clone();
    let key = app.image_key(&app.images.slots[0]).context("Inline key")?;
    let start = app.images.slots[0].rows.start;
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    for offset in (0..12).chain((0..12).rev()) {
        app.scroll = start + offset;
        app.poll_images();
        assert!(!app.images_pending());
        assert_eq!(app.image_key(&app.images.slots[0]), Some(key.clone()));
        terminal.draw(|frame| ui::draw(frame, &app))?;
        assert_eq!(native_rows(terminal.backend().buffer()), 12 - offset);
    }
    assert_eq!(app.reader_text, text);
    Ok(())
}

#[test]
fn help_and_theme_changes_restore_native_rows_without_covering_controls()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    app.mode = Mode::Help;
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    assert_eq!(native_rows(terminal.backend().buffer()), 0);

    app.mode = Mode::Normal;
    app.workspace.theme = "nord".into();
    settle(&mut app)?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(terminal.backend().buffer()), 12);
    Ok(())
}
