//! Exact source/layout reuse is checked before measuring rapid back navigation.
use anyhow::{Result, ensure};
use lume::reader::LiveRenderer;
use std::time::{Duration, Instant};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let copies = if stress { 2000 } else { 200 };
    let bodies = ["First", "Second", "Third"].map(|name| {
        format!(
            "# {name}\n\n{}",
            "Readable text with Unicode café and systems.\n\n".repeat(copies)
        )
    });
    let mut renderer = LiveRenderer::start()?;
    renderer.enable_background_layout()?;
    for body in &bodies {
        warm(&mut renderer, body)?;
    }
    super::measure(
        "three recent layouts: cached back navigation",
        iterations,
        || {
            for (index, body) in bodies.iter().enumerate() {
                let rendered = renderer.render(body, 80);
                ensure!(
                    rendered
                        .text
                        .starts_with(["First", "Second", "Third"][index]),
                    "Cached source drifted"
                );
                ensure!(
                    rendered.text.matches("café").count() == copies,
                    "Publisher text lost"
                );
            }
            Ok(())
        },
    )
}

fn warm(renderer: &mut LiveRenderer, body: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if !renderer.render(body, 80).text.is_empty() {
            return Ok(());
        }
        renderer.poll();
        ensure!(Instant::now() < deadline, "Layout did not finish");
        std::thread::yield_now();
    }
}
