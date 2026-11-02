//! Real App admission, cache, worker delivery and opt-in production diagnostics.
use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode},
    media::{ImageProtocol, cache},
};
use ratatui_image::protocol::Protocol;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{borrow::Cow, path::PathBuf, time::Instant};

struct Fixture {
    bytes: Cow<'static, [u8]>,
    label: String,
    digest: String,
    external: bool,
}

impl Fixture {
    fn load() -> Result<Self> {
        let (bytes, label, external) =
            match std::env::var_os("LUME_BENCH_SIXEL_INPUT") {
                Some(path) => {
                    let path = PathBuf::from(path);
                    let bytes = std::fs::read(&path).with_context(|| {
                        format!(
                            "Read LUME_BENCH_SIXEL_INPUT {}",
                            path.display()
                        )
                    })?;
                    (Cow::Owned(bytes), path.display().to_string(), true)
                }
                None => (
                    Cow::Borrowed(
                        include_bytes!(
                            "../../tests/fixtures/images/astronaut.png"
                        )
                        .as_slice(),
                    ),
                    "NASA photo".into(),
                    false,
                ),
            };
        let digest = digest(bytes.as_ref());
        Ok(Self {
            bytes,
            label,
            digest,
            external,
        })
    }
}

pub fn run(iterations: usize, _stress: bool) -> Result<()> {
    let fixture = Fixture::load()?;
    for _ in 0..iterations.min(3) {
        inspect(&fixture)?;
    }
    Ok(())
}

fn inspect(fixture: &Fixture) -> Result<()> {
    let (directory, mut app) = super::image_app()?;
    app.configure_image_terminal(ImageProtocol::Sixel, (10, 20));
    let key = app.image_key(&app.images.slots[0]).context("Inline key")?;
    let images = directory.path().join("images");
    cache::write(&images, &key.url, fixture.bytes.as_ref())?;
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
    let (wire_sha256, wire_bytes) = native_wire(&app)?;
    let report = report(&mut app, !fixture.external)?;
    println!(
        "production-profile: {}",
        json!({"case": fixture.label,
            "input_sha256": fixture.digest, "input_bytes": fixture.bytes.len(),
            "external_fixture": fixture.external,
            "native_wire_sha256": wire_sha256, "native_wire_bytes": wire_bytes,
            "inspection_delivery_ms": delivery_ms,
            "report_lines": report})
    );
    Ok(())
}

fn native_wire(app: &App) -> Result<(String, usize)> {
    let key = app.image_inspector_key().context("Inspector image key")?;
    let thumbnail = app.images.thumbnail(&key).context("Ready inspector")?;
    let Some(Protocol::Sixel(native)) = &thumbnail.native else {
        anyhow::bail!("Inspector did not receive native Sixel pixels");
    };
    Ok((digest(native.data.as_bytes()), native.data.len()))
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn report(app: &mut App, require_quantization: bool) -> Result<Vec<String>> {
    press(app, KeyCode::Esc)?;
    command(app, "performance status")?;
    let Mode::Document(report) = &app.mode else {
        anyhow::bail!("Performance report missing");
    };
    let text = report.lines.join("\n");
    for stage in ["ImageDecode", "ImageFit", "ImageEncode", "ImageWork"] {
        ensure!(text.contains(stage), "Production profile lacks {stage}");
    }
    if require_quantization {
        for stage in ["ImagePalette", "ImageDither"] {
            ensure!(text.contains(stage), "Production profile lacks {stage}");
        }
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
