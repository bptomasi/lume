//! Linked pages reuse visible-image preparation without mutating their parent.

mod common;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::{Action, App, Focus, Mode},
    media::{self, ImageKey, ImageProtocol},
    model::WebDocument,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

const URL: &str = "https://example.org/pages/linked";
const SVG: &[u8] = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><path fill="#ff0000" d="M0 0h20v10H0z"/><path fill="#0000ff" d="M0 10h20v10H0z"/></svg>"##;

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

fn fixture(
    body: &str,
    protocol: ImageProtocol,
) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = format!("[Open page]({URL})");
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let parent = store.library()?.articles[0].id;
    store.save_web_document(&WebDocument {
        url: URL.into(),
        title: "Linked diagram".into(),
        body: body.into(),
    })?;
    let mut app = App::new(store, false)?;
    app.jump_article(parent)?;
    app.focus = Focus::Reader;
    app.configure_image_terminal(protocol, (10, 20));
    app.resize_workspace(Rect::new(0, 0, 100, 36));
    let cache = directory.path().join("images");
    media::cache::write(&cache, "https://example.org/diagram.svg", SVG)?;
    app.enable_images(cache, true)?;
    Ok((directory, app))
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        app.poll_web()?;
        app.poll_images();
        if !app.web_pending() && !app.images_pending() {
            return Ok(());
        }
        ensure!(
            Instant::now() < deadline,
            "Linked image preparation timed out"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn image_key(app: &App) -> Result<ImageKey> {
    let slot = app
        .web_page()
        .context("Linked page")?
        .rendered
        .images
        .first()
        .context("Image slot")?;
    app.image_key(slot).context("Image key")
}

#[test]
fn enter_on_article_link_loads_and_paints_the_linked_image() -> Result<()> {
    let (_directory, mut app) = fixture(
        "![Network lab](../diagram.svg)\n\nAfter image.",
        ImageProtocol::Blocks,
    )?;
    let parent = app.current().context("Parent")?.id;
    let before = (app.reader_text.clone(), app.cursor_offset(), app.scroll);
    key(&mut app, KeyCode::Enter)?;
    settle(&mut app)?;
    let request = image_key(&app)?;
    assert_eq!(request.url, "https://example.org/diagram.svg");
    assert_eq!(
        request.width as usize,
        app.web_page().context("Page")?.width
    );
    assert!(app.images.thumbnail(&request).is_some());
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert!(
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .any(|cell| cell.bg == Color::Rgb(255, 0, 0))
    );
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.current().context("Restored parent")?.id, parent);
    assert_eq!(
        (app.reader_text.clone(), app.cursor_offset(), app.scroll),
        before
    );
    Ok(())
}

#[test]
fn image_enter_and_mouse_inspection_return_to_the_linked_reading() -> Result<()>
{
    let (_directory, mut app) = fixture(
        "![Network lab](/diagram.svg)\n\nAfter.",
        ImageProtocol::Sixel,
    )?;
    command(&mut app, &format!("open-page {URL}"))?;
    settle(&mut app)?;
    let cursor = app.web_page().context("Page")?.cursor;
    key(&mut app, KeyCode::Enter)?;
    assert!(
        matches!(&app.mode, Mode::Image(image) if image.url == "https://example.org/diagram.svg")
    );
    settle(&mut app)?;
    key(&mut app, KeyCode::Esc)?;
    assert!(matches!(app.mode, Mode::Web));
    assert_eq!(app.web_page().context("Page")?.cursor, cursor);
    let page = app.web_page().context("Page")?;
    let area = ui::web_content(Rect::new(0, 0, 100, 36), page.width);
    let row = area.y + page.rendered.images[0].caption as u16;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x,
        row,
        modifiers: KeyModifiers::NONE,
    })?;
    assert!(matches!(app.mode, Mode::Image(_)));
    key(&mut app, KeyCode::Esc)?;
    assert!(matches!(app.mode, Mode::Web));
    assert_eq!(
        command(&mut app, "image-open")?,
        Action::Open("https://example.org/diagram.svg".into())
    );
    Ok(())
}

#[test]
fn sixel_linked_rows_are_clipped_and_hidden_beneath_overlays() -> Result<()> {
    let (_directory, mut app) = fixture(
        &format!(
            "![Network lab](/diagram.svg)\n\n{}",
            "After.\n\n".repeat(40)
        ),
        ImageProtocol::Sixel,
    )?;
    command(&mut app, &format!("open-page {URL}"))?;
    settle(&mut app)?;
    let request = image_key(&app)?;
    assert!(
        app.images
            .thumbnail(&request)
            .context("Thumbnail")?
            .rows
            .is_some()
    );
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(&terminal), 12);
    for _ in 0..6 {
        app.mouse(MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 50,
            row: 15,
            modifiers: KeyModifiers::NONE,
        })?;
    }
    settle(&mut app)?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(&terminal), 7);
    key(&mut app, KeyCode::Char('?'))?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(&terminal), 0);
    key(&mut app, KeyCode::Esc)?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(&terminal), 7);
    Ok(())
}

fn native_rows(terminal: &Terminal<TestBackend>) -> usize {
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .filter(|cell| cell.symbol().contains("\x1bP"))
        .count()
}

#[test]
fn offscreen_images_wait_and_resize_preserves_source_mappings() -> Result<()> {
    let body = format!(
        "[Next](https://example.org/next)\n\n{}![Later](/diagram.svg)\n\n[Section](#target)\n\n## Target\n\nDestination.",
        "Earlier.\n\n".repeat(40)
    );
    let (_directory, mut app) = fixture(&body, ImageProtocol::Blocks)?;
    command(&mut app, &format!("open-page {URL}"))?;
    settle(&mut app)?;
    assert!(app.images.thumbnail(&image_key(&app)?).is_none());
    key(&mut app, KeyCode::Char('G'))?;
    settle(&mut app)?;
    assert!(app.images.thumbnail(&image_key(&app)?).is_some());
    app.resize_workspace(Rect::new(0, 0, 60, 24));
    app.poll_web()?;
    settle(&mut app)?;
    let page = app.web_page().context("Reflowed page")?;
    let slot = &page.rendered.images[0];
    assert_eq!(
        &page.document.body[slot.source.clone()],
        "![Later](/diagram.svg)"
    );
    assert_eq!(page.rendered.text[page.cursor..].chars().next(), Some('.'));
    assert_eq!(image_key(&app)?.width as usize, page.width);
    Ok(())
}

#[test]
fn links_after_images_keep_exact_anchors() -> Result<()> {
    let (_directory, mut app) = fixture(
        "![Network lab](/diagram.svg)\n\n[Jump](#target)\n\n## Target\n\nDestination.",
        ImageProtocol::Sixel,
    )?;
    command(&mut app, &format!("open-page {URL}"))?;
    settle(&mut app)?;
    for _ in 0..20 {
        if app.web_page().context("Page")?.cursor_link() == Some("#target") {
            break;
        }
        key(&mut app, KeyCode::Char('j'))?;
    }
    assert_eq!(
        app.web_page().context("Jump")?.cursor_link(),
        Some("#target")
    );
    key(&mut app, KeyCode::Enter)?;
    let page = app.web_page().context("Target")?;
    assert!(page.rendered.text[page.cursor..].starts_with("Target"));
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(
        app.web_page().context("Jump restored")?.cursor_link(),
        Some("#target")
    );
    Ok(())
}

#[test]
fn back_history_restores_cached_pixels_without_loading_them_again() -> Result<()>
{
    let (directory, mut app) = fixture(
        "[Next](https://example.org/next)\n\n![Network lab](/diagram.svg)\n\nReadable page context.",
        ImageProtocol::Sixel,
    )?;
    lume::store::Store::open(&directory.path().join("library.sqlite3"))?
        .save_web_document(&WebDocument {
            url: "https://example.org/next".into(),
            title: "Next".into(),
            body: "Next page with different content.".into(),
        })?;
    command(&mut app, &format!("open-page {URL}"))?;
    settle(&mut app)?;
    let request = image_key(&app)?;
    assert!(app.images.thumbnail(&request).is_some());
    key(&mut app, KeyCode::Enter)?;
    settle(&mut app)?;
    assert_eq!(
        app.web_page().context("Next")?.document.url,
        "https://example.org/next"
    );
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    settle(&mut app)?;
    assert_eq!(app.web_page().context("Restored page")?.document.url, URL);
    assert!(app.images.thumbnail(&image_key(&app)?).is_some());
    assert!(!app.images_pending());
    let mut terminal = Terminal::new(TestBackend::new(100, 36))?;
    terminal.draw(|frame| ui::draw_interactive(frame, &mut app))?;
    assert_eq!(native_rows(&terminal), 12);
    Ok(())
}

#[test]
fn linked_formula_pixels_and_image_caption_survive_resize() -> Result<()> {
    let (_directory, mut app) = fixture(
        "![Network lab](/diagram.svg)\n\n$$\\frac{1}{2}$$",
        ImageProtocol::Sixel,
    )?;
    command(&mut app, "latex-formulas graphics")?;
    command(&mut app, &format!("open-page {URL}"))?;
    settle(&mut app)?;
    app.resize_workspace(Rect::new(0, 0, 60, 30));
    app.poll_web()?;
    settle(&mut app)?;
    let page = app.web_page().context("Resized page")?;
    assert_eq!(page.cursor_row(), page.rendered.images[0].caption);
    let formula = page
        .rendered
        .images
        .iter()
        .find(|slot| slot.formula.is_some())
        .context("Graphical formula")?;
    let request = app.image_key(formula).context("Formula key")?;
    assert!(
        app.images
            .thumbnail(&request)
            .context("Formula pixels")?
            .rows
            .is_some()
    );
    Ok(())
}
