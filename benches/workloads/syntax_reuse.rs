//! Resizing a small article must not copy styles retained for older articles.

use std::time::{Duration, Instant};

use anyhow::{Result, ensure};
use lume::reader::{LiveRenderer, Rendered};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let blocks = if stress { 24 } else { 12 };
    let mut renderer = LiveRenderer::start()?;
    let source = (0..blocks)
        .map(|index| {
            format!(
                "```rust\n{}\n```\n\n",
                format!("let value_{index} = Some(42);\n").repeat(48)
            )
        })
        .collect::<String>();
    let original = settle(&mut renderer, &source)?;
    verify_code(&original, blocks)?;
    renderer.enable_background_layout()?;
    let prose = "A small article after a large syntax cache.";
    let ready = settle(&mut renderer, prose)?;
    ensure!(ready.text.contains(prose), "Prose was not prepared");
    println!("syntax reuse input · cached blocks={blocks} · 48 rows/block");
    let mut width = 60;
    super::super::measure(
        "resize admission with unrelated cached syntax",
        iterations * 50,
        || {
            width = if width == 60 { 80 } else { 60 };
            let rendered = renderer.render(prose, width);
            ensure!(rendered.text == ready.text, "Retained source changed");
            Ok(())
        },
    )?;
    let after = settle(&mut renderer, &source)?;
    ensure!(after.text == original.text, "Revisited code was lost");
    verify_code(&after, blocks)?;
    Ok(())
}

fn settle(renderer: &mut LiveRenderer, body: &str) -> Result<Rendered> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        renderer.poll();
        let rendered = renderer.render(body, 60);
        if !renderer.is_pending() {
            return Ok(rendered);
        }
        ensure!(Instant::now() < deadline, "Syntax preparation timed out");
        std::thread::yield_now();
    }
}

fn verify_code(rendered: &Rendered, blocks: usize) -> Result<()> {
    let rows = rendered
        .lines
        .iter()
        .zip(&rendered.code)
        .filter(|(line, code)| {
            **code && line.spans.iter().any(|span| !span.content.is_empty())
        })
        .map(|(line, _)| line)
        .collect::<Vec<_>>();
    ensure!(rows.len() == blocks * 48, "Code rows changed");
    ensure!(
        rows.iter().all(|line| {
            line.spans.iter().any(|span| span.style.fg.is_some())
        }),
        "Cached syntax lost its colors"
    );
    Ok(())
}
