//! Real format, source mapping, offline and cached-frame image contracts.

mod common;

use std::{
    io::Cursor,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use lume::{
    app::{Action, App, Focus},
    media::{self, Decoder, ImageKey},
    reader, source, ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><path fill="#ff0000" d="M0 0h2v1H0z"/><path fill="#0000ff" d="M0 1h2v1H0z"/></svg>"##;

fn key(width: u16, height: u16) -> ImageKey {
    ImageKey {
        url: "https://journal.example/diagram.svg".into(),
        width,
        height,
        background: [23, 23, 26],
        detail: None,
    }
}

fn raster(format: ImageFormat) -> Result<Vec<u8>> {
    let image = RgbaImage::from_fn(2, 2, |_, row| {
        if row == 0 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 255])
        }
    });
    let image = DynamicImage::ImageRgba8(image);
    let mut bytes = Cursor::new(Vec::new());
    let image = if format == ImageFormat::Jpeg {
        DynamicImage::ImageRgb8(image.to_rgb8())
    } else {
        image
    };
    image.write_to(&mut bytes, format)?;
    Ok(bytes.into_inner())
}

fn reading(body: &str) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body = body.into();
    store.ingest(&feed, &incoming.title, &incoming.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    let column =
        ui::workspace_reader_content(Rect::new(0, 0, 100, 36), &app.workspace);
    app.resize_reader(column.width.into(), column.height.into());
    Ok((directory, app))
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_images();
        if !app.images_pending() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Image worker timed out");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
}

#[test]
fn raster_and_svg_preserve_known_colors_without_terminal_escape_sequences()
-> Result<()> {
    let mut decoder = Decoder::default();
    for format in [ImageFormat::Png, ImageFormat::WebP, ImageFormat::Gif] {
        let image = decoder.render(&raster(format)?, &key(2, 1))?;
        assert_eq!(image.dimensions, (2, 2));
        let cell = &image.cells[(0, 0)];
        assert_eq!(cell.symbol(), "▀", "{format:?}");
        assert_eq!(cell.fg, Color::Rgb(255, 0, 0), "{format:?}");
        assert_eq!(cell.bg, Color::Rgb(0, 0, 255), "{format:?}");
    }
    let image = decoder.render(SVG, &key(2, 1))?;
    assert_eq!(image.cells[(0, 0)].fg, Color::Rgb(255, 0, 0));
    assert_eq!(image.cells[(0, 0)].bg, Color::Rgb(0, 0, 255));
    let jpeg = decoder.render(&raster(ImageFormat::Jpeg)?, &key(2, 1))?;
    assert_eq!(jpeg.dimensions, (2, 2));
    assert_ne!(jpeg.cells[(0, 0)].fg, jpeg.cells[(0, 0)].bg);
    Ok(())
}

#[test]
fn transparent_pixels_use_the_selected_matte_and_aspect_ratio_is_preserved()
-> Result<()> {
    let image = DynamicImage::ImageRgba8(RgbaImage::from_pixel(
        8,
        2,
        Rgba([255, 0, 0, 0]),
    ));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png)?;
    let thumbnail = Decoder::default().render(bytes.get_ref(), &key(16, 12))?;

    assert_eq!(
        (thumbnail.cells.area.width, thumbnail.cells.area.height),
        (16, 2)
    );
    assert!(thumbnail.cells.content.iter().all(|cell| {
        cell.symbol() == "▀" && cell.bg == Color::Rgb(23, 23, 26)
    }));
    Ok(())
}

#[test]
fn malformed_unsupported_and_oversized_images_fail_without_ready_cells()
-> Result<()> {
    let mut decoder = Decoder::default();
    assert!(decoder.render(b"not an image", &key(20, 10)).is_err());
    assert!(decoder.render(b"GIF89a", &key(20, 10)).is_err());
    assert!(
        decoder
            .render(&vec![0; media::MAX_BYTES + 1], &key(20, 10))
            .is_err()
    );
    assert!(decoder.render(SVG, &key(0, 10)).is_err());
    assert!(decoder.render(SVG, &key(161, 10)).is_err());
    let image = DynamicImage::ImageRgb8(image::RgbImage::new(8193, 1));
    let mut bytes = Cursor::new(Vec::new());
    image.write_to(&mut bytes, ImageFormat::Png)?;
    assert!(decoder.render(bytes.get_ref(), &key(20, 10)).is_err());
    Ok(())
}

#[test]
fn svg_rejects_entities_and_node_bombs_and_never_loads_referenced_files()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("private.png");
    std::fs::write(&file, raster(ImageFormat::Png)?)?;
    let svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><image href="{}" width="2" height="2"/><image href="https://invalid.example/image.png" width="2" height="2"/><script>throw new Error('never execute');</script></svg>"#,
        file.display()
    );
    let mut decoder = Decoder::default();
    let rendered = decoder.render(svg.as_bytes(), &key(2, 1))?;
    assert!(
        rendered
            .cells
            .content
            .iter()
            .all(|cell| cell.bg == Color::Rgb(23, 23, 26))
    );

    let entities = br#"<!DOCTYPE svg [<!ENTITY x "expanded">]><svg xmlns="http://www.w3.org/2000/svg"><text>&x;</text></svg>"#;
    assert!(decoder.render(entities, &key(20, 10)).is_err());
    let nodes = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg">{}</svg>"#,
        "<g/>".repeat(5001)
    );
    assert!(decoder.render(nodes.as_bytes(), &key(20, 10)).is_err());
    assert!(
        decoder
            .render(&vec![b' '; 512 * 1024 + 1], &key(20, 10))
            .is_err()
    );
    Ok(())
}

#[test]
fn html_normalization_keeps_relative_lazy_images_and_escapes_alternative_text()
{
    let html = r#"<p>Before<img src="pixel.gif" data-src="../diagram.svg" alt="a [diagram]">after</p><img src="javascript:alert(1)"><script><img src="https://bad.example/x"></script>"#;
    let body = lume::content::article_markdown(
        html,
        true,
        "https://journal.example/posts/a",
    );
    assert!(
        body.contains(
            r"![a \[diagram\]](<https://journal.example/diagram.svg>)"
        )
    );
    assert!(!body.contains("javascript") && !body.contains("bad.example"));
    assert!(body.contains("Before") && body.contains("after"));
}

#[test]
fn image_slots_preserve_source_positions_and_code_examples_are_not_images() {
    let body = "Before.\n\n![Diagram](https://example.org/a.svg)\n\nAfter.\n\n```text\n![code](https://example.org/code.png)\n```\n";
    let rendered = reader::render(body, 30);
    assert_eq!(rendered.images.len(), 1);
    let slot = &rendered.images[0];
    assert_eq!(slot.alt, "Diagram");
    assert_eq!(
        &body[slot.source.clone()],
        "![Diagram](https://example.org/a.svg)"
    );
    assert_eq!(slot.rows.len(), 12);
    assert!(rendered.lines[slot.caption].to_string().contains("Diagram"));
    assert!(
        rendered.lines[slot.rows.end..]
            .iter()
            .any(|line| line.to_string() == "After.")
    );
}

#[test]
fn cached_image_completion_does_not_change_text_selection_or_scroll()
-> Result<()> {
    let (directory, mut app) =
        reading("Before.\n\n![Diagram](/diagram.svg)\n\nAfter.")?;
    let slot = app.images.slots[0].clone();
    let key = app.image_key(&slot).context("Image key")?;
    assert_eq!(key.url, "https://journal.example/diagram.svg");
    let cache = directory.path().join("images");
    media::cache::write(&cache, &key.url, SVG)?;
    app.selection = Some(app.cursor_offset());
    let before = (app.reader_text.clone(), app.cursor_offset(), app.scroll);

    app.enable_images(cache, true)?;
    settle(&mut app)?;

    assert!(app.images.thumbnail(&key).is_some());
    assert_eq!(
        (app.reader_text.clone(), app.cursor_offset(), app.scroll),
        before
    );
    assert_eq!(app.selection, Some(before.1));
    assert_eq!(app.images.slots[0].rows, slot.rows);
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    for color in [Color::Rgb(255, 0, 0), Color::Rgb(0, 0, 255)] {
        assert!(
            terminal
                .backend()
                .buffer()
                .content
                .iter()
                .any(|cell| cell.bg == color),
            "Missing image color {color:?}"
        );
    }
    Ok(())
}

#[test]
fn offline_miss_can_be_retried_and_loading_can_be_paused() -> Result<()> {
    let (directory, mut app) = reading("![Diagram](/diagram.svg)")?;
    let key = app.image_key(&app.images.slots[0]).context("Image key")?;
    let cache = directory.path().join("images");
    command(&mut app, "images off")?;
    app.enable_images(cache.clone(), true)?;
    assert!(!app.poll_images() && !app.images_pending());
    command(&mut app, "images on")?;
    settle(&mut app)?;
    assert!(
        app.images
            .error(&key)
            .context("Offline error")?
            .contains("offline")
    );
    media::cache::write(&cache, &key.url, SVG)?;

    command(&mut app, "images retry")?;
    settle(&mut app)?;

    assert!(app.images.thumbnail(&key).is_some());
    app.reader_cursor = app.images.slots[0].caption;
    assert_eq!(command(&mut app, "image-open")?, Action::Open(key.url));
    app.focus = Focus::Articles;
    assert!(command(&mut app, "image-open").is_err());
    Ok(())
}

#[test]
fn cache_evicts_only_owned_files_and_never_uses_url_paths() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let marker = directory.path().join("keep.txt");
    std::fs::write(&marker, "Keep unrelated files")?;
    for index in 0..130 {
        media::cache::write(
            directory.path(),
            &format!("https://example.org/{index}"),
            SVG,
        )?;
    }
    let count = std::fs::read_dir(directory.path())?.count();
    assert_eq!(count, 129);
    assert_eq!(std::fs::read_to_string(marker)?, "Keep unrelated files");
    let path = media::cache::path(
        directory.path(),
        "https://example.org/../../private",
    );
    assert_eq!(path.parent(), Some(directory.path()));
    assert_eq!(path.file_name().context("Cache filename")?.len(), 68);
    Ok(())
}

#[test]
fn theme_changes_keep_literal_image_colors_and_image_rows_keep_a_visible_cursor()
-> Result<()> {
    let (directory, mut app) = reading("![Dark diagram](/diagram.svg)")?;
    let cache = directory.path().join("images");
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="2" height="2"><path fill="#17171a" d="M0 0h2v2H0z"/></svg>"##;
    let key = app.image_key(&app.images.slots[0]).context("Image key")?;
    media::cache::write(&cache, &key.url, svg)?;
    app.enable_images(cache, true)?;
    let column =
        ui::workspace_reader_content(Rect::new(0, 0, 100, 36), &app.workspace);
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    for theme in ["lume", "nord", "gruvbox"] {
        app.workspace.theme = theme.into();
        settle(&mut app)?;
        let slot = &app.images.slots[0];
        let key = app.image_key(slot).context("Themed image key")?;
        let image = app.images.thumbnail(&key).context("Prepared image")?;
        let x = column.x + (column.width - image.cells.area.width) / 2;
        let y = column.y + slot.rows.start as u16;
        app.reader_cursor = slot.rows.start;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        assert_eq!(
            terminal.backend().buffer()[(x, y)].bg,
            Color::Rgb(23, 23, 26)
        );
        let cursor = &terminal.backend().buffer()[(column.x, y)];
        assert_ne!(cursor.bg, Color::Rgb(23, 23, 26));
        assert!(cursor.modifier.contains(ratatui::style::Modifier::BOLD));
    }
    Ok(())
}

#[test]
fn resizing_uses_a_new_thumbnail_key_without_mutating_text_or_resolving_an_invalid_scheme()
-> Result<()> {
    let (directory, mut app) = reading("![Diagram](/diagram.svg)\n\nAfter.")?;
    let old = app
        .image_key(&app.images.slots[0])
        .context("Original key")?;
    let cache = directory.path().join("images");
    media::cache::write(&cache, &old.url, SVG)?;
    app.enable_images(cache, true)?;
    settle(&mut app)?;
    let text = app.reader_text.clone();

    app.resize_reader(8, 3);
    let new = app.image_key(&app.images.slots[0]).context("Narrow key")?;
    assert_ne!(new, old);
    assert!(app.images.thumbnail(&new).is_none());
    settle(&mut app)?;

    assert!(
        app.images
            .thumbnail(&new)
            .context("Narrow image")?
            .cells
            .area
            .width
            <= 8
    );
    assert_eq!(app.reader_text, text);
    let (_directory, mut invalid) = reading("![Private](file:///etc/passwd)")?;
    assert!(invalid.images.slots[0].url.is_empty());
    invalid.enable_images(directory.path().join("invalid"), true)?;
    assert!(!invalid.poll_images() && !invalid.images_pending());
    assert!(command(&mut invalid, "image-open").is_err());
    Ok(())
}

#[test]
fn gif_animations_decode_only_the_first_frame() -> Result<()> {
    let mut bytes = Vec::new();
    {
        let mut encoder = image::codecs::gif::GifEncoder::new(&mut bytes);
        for color in [Rgba([255, 0, 0, 255]), Rgba([0, 0, 255, 255])] {
            encoder.encode_frame(image::Frame::new(RgbaImage::from_pixel(
                2, 2, color,
            )))?;
        }
    }
    let image = Decoder::default().render(&bytes, &key(2, 1))?;
    assert!(image.cells.content.iter().all(|cell| {
        cell.fg == Color::Rgb(255, 0, 0) && cell.bg == Color::Rgb(255, 0, 0)
    }));
    Ok(())
}

#[test]
fn disk_cache_obeys_its_byte_budget_and_ignores_non_cache_unicode_names()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let unrelated = directory.path().join(format!("{}.img", "界".repeat(21)));
    std::fs::write(&unrelated, b"Keep unrelated bytes")?;
    let bytes = vec![0; media::MAX_BYTES];
    for index in 0..9 {
        media::cache::write(
            directory.path(),
            &format!("https://example.org/{index}"),
            &bytes,
        )?;
    }
    let mut total = 0;
    for entry in std::fs::read_dir(directory.path())? {
        let entry = entry?;
        if entry.path() != unrelated {
            total += entry.metadata()?.len();
        }
    }
    assert!(total <= 64 * 1024 * 1024);
    assert_eq!(std::fs::read(unrelated)?, b"Keep unrelated bytes");
    assert!(
        media::cache::write(
            directory.path(),
            "https://example.org/too-big",
            &vec![0; media::MAX_BYTES + 1]
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn collapsed_reader_does_not_admit_images_until_it_is_visible() -> Result<()> {
    let (directory, mut app) = reading("![Diagram](/diagram.svg)")?;
    let key = app.image_key(&app.images.slots[0]).context("Image key")?;
    let cache = directory.path().join("images");
    media::cache::write(&cache, &key.url, SVG)?;
    app.enable_images(cache, true)?;
    app.workspace.fullscreen = false;
    app.workspace.show_reader = false;

    app.poll_images();

    assert!(!app.images_pending());
    assert!(app.images.thumbnail(&key).is_none());
    app.workspace.show_reader = true;
    settle(&mut app)?;
    assert!(app.images.thumbnail(&key).is_some());
    Ok(())
}
