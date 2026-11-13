//! Native Foot strips preserve pixels and reader identity through clipped scroll.

mod common;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use lume::frame_composition::draw_interactive;
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
        assert!(actual.contains("\x1bP"));
        assert!(actual.contains("\"1;1;800;20"));
        assert!(clipped[(4, 2 + row)].skip);
    }
    assert_eq!(clipped[(2, 2)].symbol(), " ");
    assert_eq!(clipped[(3, 6)].symbol(), " ");
    let payload_bytes = (2..14)
        .map(|row| complete[(3, row)].symbol().len())
        .sum::<usize>();
    assert_eq!(
        rows.bytes(),
        payload_bytes + 12 * std::mem::size_of::<ratatui::buffer::Cell>()
    );
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

#[test]
fn strips_erase_previous_text_in_partially_covered_edge_cells() -> Result<()> {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"
        width="101" height="240">
        <path fill="white" d="M0 0h101v240H0z"/>
    </svg>"#;
    let mut narrow = key();
    narrow.width = 11;
    let image = Decoder::default().render(svg, &narrow)?;
    let rows = image.rows.context("Native Sixel strips")?;
    assert_eq!(image.cells.area.width, 11);
    let area = Rect::new(3, 2, 11, image.cells.area.height);
    let mut previous = Buffer::empty(Rect::new(0, 0, 20, 16));
    for cell in &mut previous.content {
        cell.set_symbol("i");
    }
    let mut next = Buffer::empty(previous.area);
    let background = ratatui::style::Color::Rgb(46, 52, 64);
    next.set_style(area, ratatui::style::Style::new().bg(background));
    rows.render(area, 0, &mut next);
    let updates = previous.diff(&next);
    for y in area.y..area.bottom() {
        let (_, _, cell) = updates
            .iter()
            .find(|(x, row, _)| *x == area.x && *row == y)
            .context("Strip missing from terminal updates")?;
        assert!(cell.symbol().starts_with("\x1b[11X\x1bP"));
        assert!(cell.symbol().contains("\"1;1;101;"));
        assert_eq!(cell.bg, background);
        assert!(next[(area.right() - 1, y)].skip);
    }
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
    app.activate_pane(Focus::Reader);
    common::fixture_toggle(&mut app, "fullscreen", true)?;
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
        .filter(|cell| cell.symbol().contains("\x1bP"))
        .count()
}

#[test]
fn both_scroll_directions_keep_the_same_prepared_image_and_source() -> Result<()>
{
    let (_directory, mut app) = reading()?;
    let text = app.reader().reader_text.to_owned();
    let key = app
        .image_key(&app.reader().image_slots[0])
        .context("Inline key")?;
    let start = app.reader().image_slots[0].rows.start;
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    for offset in (0..12).chain((0..12).rev()) {
        let scroll = start + offset;
        common::reader::scroll(&mut app, scroll)?;
        app.poll_images();
        assert!(!app.images_pending());
        assert_eq!(
            app.image_key(&app.reader().image_slots[0]),
            Some(key.clone())
        );
        terminal.draw(|frame| ui::draw(frame, &app.presentation()))?;
        assert_eq!(native_rows(terminal.backend().buffer()), 12 - offset);
    }
    assert_eq!(app.reader().reader_text, text);
    Ok(())
}

#[test]
fn help_and_theme_changes_restore_native_rows_without_covering_controls()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    app.key(crossterm::event::KeyCode::Char('?').into())?;
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| ui::draw(frame, &app.presentation()))?;
    assert_eq!(native_rows(terminal.backend().buffer()), 0);

    app.key(crossterm::event::KeyCode::Esc.into())?;
    common::fixture_command(&mut app, "theme nord")?;
    settle(&mut app)?;
    terminal.draw(|frame| draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(terminal.backend().buffer()), 12);
    Ok(())
}

fn press(
    app: &mut App,
    code: crossterm::event::KeyCode,
) -> Result<lume::app::Action> {
    use crossterm::event::{KeyEvent, KeyModifiers};

    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn after_image_row(app: &App) -> Result<usize> {
    app.reader()
        .reader_lines
        .iter()
        .position(|line| line == "After image.")
        .context("Text after image")
}

fn assert_native(app: &mut App) -> Result<()> {
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| draw_interactive(frame, app))?;
    assert_eq!(native_rows(terminal.backend().buffer()), 12);
    assert!(!app.images_pending());
    Ok(())
}

#[test]
fn character_and_linewise_selection_keep_images_and_yank_exact_text()
-> Result<()> {
    use crossterm::event::KeyCode;
    use lume::app::Action;

    let (_directory, mut app) = reading()?;
    let text = app.reader().reader_text.to_owned();
    let after = after_image_row(&app)?;
    let row = after;
    let byte = 0;
    common::reader::cursor(&mut app, row, byte)?;
    for code in ['v', 'V'] {
        if app.reader().selection.is_some() {
            app.key(crossterm::event::KeyCode::Esc.into())?;
        }
        press(&mut app, KeyCode::Char(code))?;
        assert!(app.reader().selection.is_some());
        assert_native(&mut app)?;
        let expected = if code == 'v' { "A" } else { "After image." };
        assert_eq!(
            press(&mut app, KeyCode::Char('y'))?,
            Action::Copy(expected.into())
        );
    }
    assert_eq!(app.reader().reader_text, text);
    Ok(())
}

#[test]
fn mouse_click_and_drag_keep_inline_pixels_through_every_selection_frame()
-> Result<()> {
    use crossterm::event::{
        KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
    };
    use lume::app::Action;

    let (_directory, mut app) = reading()?;
    let reader = ui::workspace_areas(
        Rect::new(0, 0, 100, 36),
        app.focus(),
        app.workspace(),
    )
    .reader
    .context("Reader")?;
    let column = ui::workspace_reader_content(reader, app.workspace());
    let after = after_image_row(&app)?;
    let row = column.y + after as u16;
    for (kind, distance) in [
        (MouseEventKind::Down(MouseButton::Left), 0),
        (MouseEventKind::Up(MouseButton::Left), 0),
        (MouseEventKind::Down(MouseButton::Left), 0),
        (MouseEventKind::Drag(MouseButton::Left), 4),
        (MouseEventKind::Up(MouseButton::Left), 4),
    ] {
        app.mouse(MouseEvent {
            kind,
            column: column.x + distance,
            row,
            modifiers: KeyModifiers::NONE,
        })?;
        assert!(matches!(app.mode(), Mode::Normal));
        assert_native(&mut app)?;
    }
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("After".into())
    );
    Ok(())
}

#[test]
fn layout_prefix_help_preserves_pixels_above_its_footprint() -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let (_directory, mut app) = reading()?;
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    assert!(app.layout_prefix_pending());
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(terminal.backend().buffer()), 12);
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
            .contains("Split below")
    );

    press(&mut app, KeyCode::Esc)?;
    assert_native(&mut app)?;
    Ok(())
}

#[test]
fn popup_occlusion_hides_only_intersecting_strips() -> Result<()> {
    let image = Decoder::default().render(SVG, &key())?;
    let rows = image.rows.context("Native Sixel strips")?;
    let mut cells = Buffer::empty(Rect::new(0, 0, 100, 20));
    cells[(10, 5)].set_symbol("Popup");
    let popup = Rect::new(10, 5, 8, 3);

    rows.render_uncovered(Rect::new(3, 2, 80, 12), 0, &mut cells, &[popup]);

    assert_eq!(native_rows(&cells), 9);
    assert_eq!(cells[(10, 5)].symbol(), "Popup");
    for y in 2..14 {
        assert_eq!(
            cells[(3, y)].symbol().contains("\x1bP"),
            !(5..8).contains(&y)
        );
        assert_eq!(cells[(4, y)].skip, !(5..8).contains(&y));
    }
    Ok(())
}

#[test]
fn layout_guide_below_an_image_leaves_that_image_visible() -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_directory, mut app) = reading()?;
    app.resize_workspace(Rect::new(0, 0, 100, 60));
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    let mut terminal = Terminal::new(TestBackend::new(100, 60))?;

    terminal.draw(|frame| draw_interactive(frame, &mut app))?;

    assert_eq!(native_rows(terminal.backend().buffer()), 12);
    Ok(())
}
