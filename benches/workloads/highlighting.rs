//! Cold loading and repeated layout of original, mixed-language articles.

#[path = "../../tests/fixtures/code_samples.rs"]
mod samples;

use std::{hint::black_box, time::Instant};

use anyhow::{Result, ensure};
use lume::reader::{self, Rendered};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let repetitions = if stress { 100 } else { 20 };
    let (source, expected) = article(repetitions);
    let started = Instant::now();
    let rendered = reader::render(&source, 78);
    let first_ms = started.elapsed().as_secs_f64() * 1000.0;
    verify(&rendered, &expected)?;
    verify_colors()?;
    println!(
        "polyglot input: {} bytes, {} code lines, first render {first_ms:.3} ms",
        source.len(),
        expected.len(),
    );
    super::measure("15-language article layout", iterations, || {
        let rendered = reader::render(black_box(&source), 78);
        verify(&rendered, &expected)
    })
}

fn article(repetitions: usize) -> (String, Vec<&'static str>) {
    let mut source = String::new();
    let mut expected = Vec::new();
    for _ in 0..repetitions {
        for sample in samples::SAMPLES {
            source.push_str(&format!(
                "## {}\n\n```{}\n{}```\n\n",
                sample.language, sample.language, sample.code,
            ));
            expected.extend(sample.code.lines());
        }
    }
    (source, expected)
}

fn verify(rendered: &Rendered, expected: &[&str]) -> Result<()> {
    let actual = rendered
        .lines
        .iter()
        .zip(&rendered.code)
        .filter(|(_, code)| **code)
        .map(|(line, _)| line.to_string())
        .collect::<Vec<_>>();
    ensure!(actual == expected, "Mixed-language layout changed code");
    Ok(())
}

fn verify_colors() -> Result<()> {
    for sample in samples::SAMPLES {
        let markdown = format!("```{}\n{}```", sample.language, sample.code);
        let rendered = reader::render(&markdown, 78);
        ensure!(
            rendered.lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains(sample.token)
                        && span.style.fg == Some(sample.color)
                })
            }),
            "{} lost its colored {} token",
            sample.language,
            sample.token,
        );
    }
    Ok(())
}
