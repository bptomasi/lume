//! Linked reflow and selected frames retain content and source identity.

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{app::App, media, model::WebDocument, ui};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let sections = if stress { 1000 } else { 200 };
    let (directory, store) = super::reading_services::fixture(1)?;
    let section = concat!(
        "## Boundary $x^2$\n\n",
        "Readable **Unicode café 界** text with [a link](#boundary-x2).\n\n",
        "![Diagram](/diagram.svg)\n\n",
    );
    store.save_web_document(&WebDocument {
        url: "https://example.org/review".into(),
        title: "Linked review".into(),
        body: format!("{}TAILMARK", section.repeat(sections)),
    })?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 120, 40));
    let cache = directory.path().join("images");
    media::cache::write(
        &cache,
        "https://example.org/diagram.svg",
        include_bytes!("../../tests/fixtures/images/request-path.svg"),
    )?;
    app.enable_images(cache, true)?;
    key(&mut app, KeyCode::Char(':'))?;
    app.paste("open-page https://example.org/review")?;
    key(&mut app, KeyCode::Enter)?;
    settle(&mut app)?;
    key(&mut app, KeyCode::Char('G'))?;
    key(&mut app, KeyCode::Char('b'))?;
    reflow(&mut app, iterations)?;
    selected_frames(&mut app, iterations)?;
    Ok(())
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        app.poll_web()?;
        app.poll_images();
        if !app.web_pending() && !app.images_pending() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Linked worker timed out");
        std::thread::yield_now();
    }
}

fn reflow(app: &mut App, iterations: usize) -> Result<()> {
    super::measure(
        "linked two-width reflow + anchor restoration",
        iterations,
        || {
            for width in [78, 120] {
                app.resize_workspace(Rect::new(0, 0, width, 40));
                app.poll_web()?;
                settle(app)?;
                let page = app.web_page().context("Reflowed page")?;
                ensure!(
                    page.rendered.text[page.cursor..].starts_with("TAILMARK"),
                    "Deep cursor drift"
                );
                ensure!(
                    page.rendered.images.len() == 32,
                    "Image bounds changed"
                );
            }
            Ok(())
        },
    )
}

fn selected_frames(app: &mut App, iterations: usize) -> Result<()> {
    key(app, KeyCode::Char('g'))?;
    key(app, KeyCode::Char('v'))?;
    for _ in 0..18 {
        key(app, KeyCode::Char('j'))?;
    }
    settle(app)?;
    let request = app
        .image_key(&app.web_page().context("Page")?.rendered.images[0])
        .context("Image key")?;
    ensure!(app.images.thumbnail(&request).is_some(), "Image is missing");
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
    super::measure(
        "linked selected cached-image frame",
        iterations * 10,
        || {
            terminal.draw(|frame| ui::draw_interactive(frame, app))?;
            let page = app.web_page().context("Page")?;
            ensure!(page.selected_range().is_some(), "Selection was lost");
            ensure!(
                app.images.thumbnail(&request).is_some(),
                "Image was evicted"
            );
            Ok(())
        },
    )
}
