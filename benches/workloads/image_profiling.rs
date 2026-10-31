//! Real App admission, cache, worker delivery and opt-in production diagnostics.
use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode},
    media::{ImageProtocol, cache},
};
use serde_json::json;
use std::time::Instant;

pub fn run(iterations: usize, _stress: bool) -> Result<()> {
    let photo = include_bytes!("../../tests/fixtures/images/astronaut.png");
    for _ in 0..iterations.min(3) {
        let (directory, mut app) = super::image_app()?;
        app.configure_image_terminal(ImageProtocol::Sixel, (10, 20));
        let key = app.image_key(&app.images.slots[0]).context("Inline key")?;
        let images = directory.path().join("images");
        cache::write(&images, &key.url, photo)?;
        command(&mut app, "performance on")?;
        press(&mut app, KeyCode::Esc)?;
        app.enable_images(images, true)?;
        super::settle(&mut app)?;
        app.reader_cursor = app.images.slots[0].caption;
        let started = Instant::now();
        press(&mut app, KeyCode::Enter)?;
        super::settle(&mut app)?;
        let delivery_ms = started.elapsed().as_secs_f64() * 1000.0;
        ensure!(
            matches!(app.mode, Mode::Image(_)),
            "Image inspector did not open"
        );
        let report = report(&mut app)?;
        println!(
            "production-profile: {}",
            json!({"case": "NASA photo", "inspection_delivery_ms": delivery_ms,
            "report_lines": report})
        );
    }
    Ok(())
}

fn report(app: &mut App) -> Result<Vec<String>> {
    press(app, KeyCode::Esc)?;
    command(app, "performance status")?;
    let Mode::Document(report) = &app.mode else {
        anyhow::bail!("Performance report missing");
    };
    let text = report.lines.join("\n");
    for stage in [
        "ImageDecode",
        "ImageFit",
        "ImagePalette",
        "ImageDither",
        "ImageEncode",
        "ImageWork",
    ] {
        ensure!(text.contains(stage), "Production profile lacks {stage}");
    }
    Ok(report.lines.clone())
}

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    press(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    press(app, KeyCode::Enter)
}
