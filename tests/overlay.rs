//! Real foreground processes qualify the optional helper without opening a GUI.

#![cfg(unix)]

mod common;

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::{App, Focus, Mode},
    media::{
        self, Decoder, ImageDetail, ImageKey, ImageProtocol, ImageSlice,
        PreparedPng,
    },
    overlay::{Configuration, Output, Placement, Runtime},
    source, ui,
};
use ratatui::layout::Rect;
use serde_json::Value;

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="128" height="64"><path fill="white" d="M0 0h128v64H0z"/><path fill="black" d="M64 0h1v64h-1z"/></svg>"#;

fn key() -> ImageKey {
    ImageKey {
        url: "https://example.org/diagram.svg".into(),
        width: 16,
        height: 8,
        background: [23, 23, 26],
        detail: Some(ImageDetail {
            zoom: 1,
            pan: [0, 0],
            protocol: ImageProtocol::Overlay,
            cell_pixels: (10, 20),
            slice: None,
        }),
    }
}

fn image() -> Result<Arc<PreparedPng>> {
    Decoder::default()
        .render(SVG, &key())?
        .overlay
        .context("Prepared PNG missing")
}

fn helper(mode: &str) -> Result<(tempfile::TempDir, Configuration)> {
    let directory = tempfile::tempdir()?;
    let executable = directory.path().join("ueberzugpp");
    fs::write(
        &executable,
        include_str!("fixtures/images/overlay_helper.py"),
    )?;
    fs::set_permissions(&executable, fs::Permissions::from_mode(0o755))?;
    fs::write(directory.path().join("mode"), mode)?;
    let configuration = Configuration {
        executable,
        output: Output::X11,
        isolation: None,
    };
    Ok((directory, configuration))
}

fn placement(image: &Arc<PreparedPng>, x: u16) -> Placement {
    Placement {
        id: "lume-test-\"quoted\"".into(),
        area: Rect::new(x, 3, 16, 4),
        image: image.clone(),
        crop: None,
    }
}

fn journal(directory: &Path) -> Result<Vec<Value>> {
    let text = match fs::read_to_string(directory.join("journal.jsonl")) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Vec::new());
        }
        Err(error) => return Err(error.into()),
    };
    text.lines()
        .map(|line| serde_json::from_str(line).map_err(Into::into))
        .collect()
}

fn await_action(
    runtime: &mut Runtime,
    directory: &Path,
    action: &str,
) -> Result<Vec<Value>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        runtime.poll(Instant::now())?;
        let entries = journal(directory)?;
        if entries.iter().any(|entry| entry["action"] == action) {
            return Ok(entries);
        }
        ensure!(Instant::now() < deadline, "Helper did not observe {action}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn await_position(
    runtime: &mut Runtime,
    directory: &Path,
    x: u16,
) -> Result<Vec<Value>> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        runtime.poll(Instant::now())?;
        let entries = journal(directory)?;
        if entries
            .iter()
            .any(|entry| entry["action"] == "add" && entry["x"] == x)
        {
            return Ok(entries);
        }
        ensure!(Instant::now() < deadline, "Helper did not reach column {x}");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn await_error(runtime: &mut Runtime) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Err(error) = runtime.poll(Instant::now()) {
            return Ok(error.to_string());
        }
        ensure!(Instant::now() < deadline, "Helper failure was not reported");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn overlay_prepares_real_pixels_instead_of_a_halfblock_thumbnail() -> Result<()>
{
    let prepared = Decoder::default().render(SVG, &key())?;
    assert!(prepared.native.is_none());
    let file = prepared.overlay.context("Overlay PNG")?;
    let decoded = image::open(file.path())?.to_rgb8();
    assert_eq!((decoded.width(), decoded.height()), (160, 80));
    assert_eq!(decoded.get_pixel(80, 30).0, [0, 0, 0]);
    assert_eq!(decoded.get_pixel(70, 30).0, [255, 255, 255]);
    assert_eq!(file.bytes(), fs::metadata(file.path())?.len());
    let path = file.path().to_path_buf();
    drop(file);
    assert!(!path.exists());
    Ok(())
}

#[test]
fn partial_inline_images_crop_pixels_without_squeezing_the_whole_diagram()
-> Result<()> {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="120"><path fill="red" d="M0 0h100v60H0z"/><path fill="blue" d="M0 60h100v60H0z"/></svg>"#;
    let mut request = key();
    request.width = 20;
    request.height = 6;
    request.detail.as_mut().context("Detail")?.slice = Some(ImageSlice {
        first_row: 3,
        rows: 2,
    });
    let prepared = Decoder::default().render(svg, &request)?;
    let file = prepared.overlay.context("Cropped PNG")?;
    let decoded = image::open(file.path())?.to_rgb8();
    assert_eq!((decoded.width(), decoded.height()), (100, 40));
    assert!(decoded.pixels().all(|pixel| pixel.0 == [0, 0, 255]));
    request.detail.as_mut().context("Detail")?.slice = Some(ImageSlice {
        first_row: 5,
        rows: 2,
    });
    assert!(Decoder::default().render(svg, &request).is_err());
    Ok(())
}

#[test]
fn odd_native_pixel_height_stays_inside_the_exact_cell_budget() -> Result<()> {
    let mut request = key();
    request.height = 7;
    let detail = request.detail.as_mut().context("Detail")?;
    detail.cell_pixels = (10, 19);
    detail.zoom = 8;

    let prepared = Decoder::default().render(SVG, &request)?;
    let png = prepared.overlay.context("Odd-height PNG")?;
    let decoded = image::open(png.path())?;
    assert_eq!((decoded.width(), decoded.height()), (160, 133));
    assert_eq!(prepared.cells.area, Rect::new(0, 0, 16, 7));
    Ok(())
}

#[test]
fn scrolling_past_a_short_wide_image_produces_no_overlay_file() -> Result<()> {
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="1000" height="10"><path fill="red" d="M0 0h1000v10H0z"/></svg>"#;
    let mut request = key();
    request.detail.as_mut().context("Detail")?.slice = Some(ImageSlice {
        first_row: 3,
        rows: 2,
    });
    let prepared = Decoder::default().render(svg, &request)?;
    assert!(prepared.cells.area.is_empty());
    assert!(prepared.overlay.is_none());
    Ok(())
}

#[test]
fn helper_gets_only_latest_frame_with_escaped_ids_and_owned_png_paths()
-> Result<()> {
    let (directory, configuration) = helper("normal")?;
    let image = image()?;
    let mut runtime = Runtime::start(Ok(configuration))?;
    for x in 0..100 {
        runtime.stage(vec![placement(&image, x)], Instant::now());
    }
    assert!(journal(directory.path())?.is_empty());
    let entries = await_action(&mut runtime, directory.path(), "add")?;
    let adds = entries
        .iter()
        .filter(|entry| entry["action"] == "add")
        .collect::<Vec<_>>();
    assert_eq!(adds.len(), 1);
    assert_eq!(adds[0]["x"], 99);
    assert_eq!(adds[0]["y"], 3);
    assert_eq!(adds[0]["identifier"], "lume-test-\"quoted\"");
    assert_eq!(adds[0]["pixel_dimensions"], serde_json::json!([160, 80]));
    assert_eq!(adds[0]["scaler"], "contain");
    assert!(
        !adds[0]["path"]
            .as_str()
            .context("PNG path")?
            .contains("https:")
    );
    runtime.stage(vec![placement(&image, 2)], Instant::now());
    let entries = await_position(&mut runtime, directory.path(), 2)?;
    assert!(!entries.iter().any(|entry| entry["action"] == "remove"));
    runtime.suspend()?;
    Ok(())
}

fn scrolling_stream(
    runtime: &mut Runtime,
    image: &Arc<PreparedPng>,
    forward: bool,
) -> Result<()> {
    for step in 0..60 {
        let x = if forward { step } else { 120 - step };
        runtime.stage(vec![placement(image, x)], Instant::now());
        runtime.poll(Instant::now())?;
        std::thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

#[test]
fn continuous_movement_publishes_in_both_directions_without_hiding_or_restart()
-> Result<()> {
    let (directory, configuration) = helper("normal")?;
    let image = image()?;
    let mut runtime = Runtime::start(Ok(configuration))?;

    scrolling_stream(&mut runtime, &image, true)?;
    let first = journal(directory.path())?;
    let forward = first.iter().filter(|item| item["action"] == "add").count();
    assert!(
        forward >= 3,
        "Scrolling starved forward publication: {first:?}"
    );
    scrolling_stream(&mut runtime, &image, false)?;
    let final_frame = await_position(&mut runtime, directory.path(), 61)?;
    let adds = final_frame
        .iter()
        .filter(|item| item["action"] == "add")
        .count();
    assert!(adds >= forward + 3, "Reverse scrolling starved publication");
    assert_eq!(
        final_frame
            .iter()
            .filter(|item| item["event"] == "started")
            .count(),
        1
    );
    assert!(!final_frame.iter().any(|item| item["action"] == "remove"));
    runtime.suspend()?;
    Ok(())
}

#[test]
fn pending_images_keep_their_files_until_the_helper_is_stopped() -> Result<()> {
    let (directory, configuration) = helper("normal")?;
    let image = image()?;
    let path = image.path().to_path_buf();
    let mut runtime = Runtime::start(Ok(configuration))?;
    runtime.stage(vec![placement(&image, 2)], Instant::now());
    drop(image);
    let entries = await_action(&mut runtime, directory.path(), "add")?;
    let pid = entries[0]["pid"].as_u64().context("Helper PID")?;
    assert!(path.exists());
    runtime.suspend()?;
    assert!(!path.exists());
    #[cfg(target_os = "linux")]
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    Ok(())
}

#[test]
fn terminal_resize_restarts_the_helper_even_when_image_placement_is_unchanged()
-> Result<()> {
    let (directory, configuration) = helper("normal")?;
    let image = image()?;
    let mut runtime = Runtime::start(Ok(configuration))?;
    runtime.resize((144, 44), (10, 20));
    runtime.stage(vec![placement(&image, 2)], Instant::now());
    await_action(&mut runtime, directory.path(), "add")?;

    runtime.resize((160, 50), (12, 24));
    runtime.stage(vec![placement(&image, 2)], Instant::now());
    let deadline = Instant::now() + Duration::from_secs(5);
    let entries = loop {
        runtime.poll(Instant::now())?;
        let entries = journal(directory.path())?;
        let published = entries.iter().filter(|item| item["action"] == "add");
        if published.count() == 2 && !runtime.pending() {
            break entries;
        }
        ensure!(Instant::now() < deadline, "Helper did not refresh geometry");
        std::thread::sleep(Duration::from_millis(1));
    };
    let starts: Vec<_> = entries
        .iter()
        .filter(|item| item["event"] == "started")
        .collect();
    assert_eq!(starts.len(), 2);
    assert_ne!(starts[0]["pid"], starts[1]["pid"]);
    runtime.resize((160, 50), (12, 24));
    runtime.stage(vec![placement(&image, 2)], Instant::now());
    assert!(!runtime.pending());
    runtime.suspend()?;
    Ok(())
}

#[test]
fn missing_helpers_and_early_exits_report_recoverable_errors() -> Result<()> {
    let image = image()?;
    let mut unavailable =
        Runtime::start(Err("Optional helper is not installed".into()))?;
    unavailable.stage(vec![placement(&image, 0)], Instant::now());
    assert!(await_error(&mut unavailable)?.contains("not installed"));
    unavailable.suspend()?;
    let (_directory, configuration) = helper("exit")?;
    let mut exited = Runtime::start(Ok(configuration))?;
    exited.stage(vec![placement(&image, 0)], Instant::now());
    let error = await_error(&mut exited)?;
    assert!(error.contains("exited") || error.contains("accepting frames"));
    exited.suspend()?;
    Ok(())
}

#[test]
fn a_helper_exit_after_first_frame_is_discovered_without_another_user_key()
-> Result<()> {
    let (directory, configuration) = helper("exit_after_frame")?;
    let image = image()?;
    let mut runtime = Runtime::start(Ok(configuration))?;
    runtime.stage(vec![placement(&image, 0)], Instant::now());
    await_action(&mut runtime, directory.path(), "add")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Err(error) =
            runtime.poll(Instant::now() + Duration::from_secs(2))
        {
            assert!(error.to_string().contains("exited"));
            break;
        }
        ensure!(Instant::now() < deadline, "Helper exit was not discovered");
        std::thread::sleep(Duration::from_millis(1));
    }
    runtime.suspend()?;
    Ok(())
}

fn reading() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body =
        "Before.\n\n![Diagram](https://example.org/diagram.svg)\n\nAfter."
            .into();
    store.ingest(&feed, &incoming.title, &incoming.articles)?;
    let cache = directory.path().join("images");
    media::cache::write(&cache, "https://example.org/diagram.svg", SVG)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.configure_image_terminal(ImageProtocol::Overlay, (10, 20));
    app.enable_images(cache, true)?;
    Ok((directory, app))
}

fn settle_images(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        if !app.images_pending() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "PNG preparation timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn reader_projections_are_clipped_and_hide_for_menus_and_selection()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    settle_images(&mut app)?;
    assert_eq!(app.overlay_placements().len(), 1);
    let slot = app.images.slots[0].clone();
    app.scroll = slot.rows.start + 3;
    settle_images(&mut app)?;
    let projected = app.overlay_placements();
    assert_eq!(projected.len(), 1);
    let column =
        ui::workspace_reader_content(Rect::new(0, 0, 144, 44), &app.workspace);
    assert_eq!(projected[0].area.y, column.y);
    assert_eq!(projected[0].area.intersection(column), projected[0].area);
    app.mode = Mode::Help;
    assert!(app.overlay_placements().is_empty());
    app.mode = Mode::Normal;
    app.selection = Some(app.cursor_offset());
    assert!(app.overlay_placements().is_empty());
    app.selection = None;
    app.image_overlay_failed("Optional helper unavailable");
    assert!(!app.uses_image_overlay());
    assert!(app.overlay_placements().is_empty());
    settle_images(&mut app)?;
    let key = app
        .image_key(&app.images.slots[0])
        .context("Fallback key")?;
    assert!(
        app.images
            .thumbnail(&key)
            .context("Fallback pixels")?
            .native
            .is_none()
    );
    Ok(())
}

#[test]
fn font_size_changes_replace_pixels_and_inspector_zoom_retains_inline_cache()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    settle_images(&mut app)?;
    let original = app.overlay_placements();
    let pixels = image::open(original[0].image.path())?;
    assert_eq!((pixels.width(), pixels.height()), (480, 240));

    app.update_image_cell_pixels((8, 16));
    assert!(app.overlay_placements().is_empty());
    settle_images(&mut app)?;
    let resized = app.overlay_placements();
    let pixels = image::open(resized[0].image.path())?;
    assert_eq!((pixels.width(), pixels.height()), (384, 192));
    let inline = app.image_key(&app.images.slots[0]).context("Inline key")?;

    app.reader_cursor = app.images.slots[0].caption;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert!(matches!(app.mode, Mode::Image(_)));
    for _ in 0..3 {
        app.key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE))?;
        settle_images(&mut app)?;
        assert_eq!(app.overlay_placements().len(), 1);
        assert!(app.images.thumbnail(&inline).is_some());
    }
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert_eq!(
        app.overlay_placements()[0].image.path(),
        resized[0].image.path()
    );
    Ok(())
}

#[test]
fn scroll_crops_keep_the_same_prepared_image_without_admitting_decode_work()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    settle_images(&mut app)?;
    let slot = app.images.slots[0].clone();
    let original = app.overlay_placements()[0].image.clone();
    let before = app.image_key(&slot).context("Original image key")?;

    for offset in (0..12).chain((0..12).rev()) {
        app.scroll = slot.rows.start + offset;
        app.poll_images();
        assert!(!app.images_pending(), "Scroll admitted image decoding");
        assert_eq!(app.image_key(&slot), Some(before.clone()));
        let placements = app.overlay_placements();
        assert_eq!(placements.len(), 1);
        assert_eq!(placements[0].image.path(), original.path());
        if offset > 0 {
            assert_eq!(placements[0].crop, Some(offset as u32 * 20..240));
        }
    }
    Ok(())
}

#[test]
fn clicking_a_clipped_image_opens_detail_and_escape_restores_reading_position()
-> Result<()> {
    let (_directory, mut app) = reading()?;
    settle_images(&mut app)?;
    app.scroll = app.images.slots[0].rows.start + 3;
    let position = (app.scroll, app.reader_cursor, app.reader_column);
    let image = app.overlay_placements()[0].area;
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: image.x + image.width / 2,
        row: image.y + 1,
        modifiers: KeyModifiers::NONE,
    };

    app.mouse(click)?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Up(MouseButton::Left),
        ..click
    })?;
    assert!(matches!(app.mode, Mode::Image(_)));
    assert!(app.selection.is_none());
    settle_images(&mut app)?;
    assert_eq!(app.overlay_placements().len(), 1);
    app.key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE))?;
    assert!(matches!(&app.mode, Mode::Image(image) if image.zoom == 2));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert_eq!((app.scroll, app.reader_cursor, app.reader_column), position);
    assert!(app.selection.is_none());
    assert_eq!(app.overlay_placements().len(), 1);
    Ok(())
}

#[test]
fn invalid_native_display_is_reported_without_blocking_the_reader() -> Result<()>
{
    let image = image()?;
    let mut runtime = Runtime::start_x11(
        "not-a-display".into(),
        42,
        lume::overlay::GridOrigin::Fixed([0, 0]),
    )?;
    runtime.resize((144, 44), (9, 18));
    runtime.stage(vec![placement(&image, 2)], Instant::now());
    assert!(!await_error(&mut runtime)?.is_empty());
    runtime.suspend()?;
    Ok(())
}
