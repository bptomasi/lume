//! Identical bounded inputs qualify full and reduced build presentation.

use std::{hint::black_box, io::Cursor, time::Instant};

use anyhow::{Result, ensure};
use lume::{media, reader};

const ARTICLE: &str = "# $x^2$\n\nBefore $x^2$ and code.\n\n\
    ```rust\nlet café = 42;\n```\n\n\
    | Name | Value |\n| --- | --- |\n| fixture | 42 |\n\n";
const COPIES: usize = 64;

fn main() -> Result<()> {
    let iterations = if std::env::args().any(|arg| arg == "--test") {
        1
    } else {
        100
    };
    println!(
        "capabilities: math={} graphics={} sixel={} svg={} syntax={} miniflux={}",
        cfg!(feature = "math"),
        cfg!(feature = "graphic-math"),
        cfg!(feature = "sixel"),
        cfg!(feature = "svg"),
        cfg!(feature = "syntax"),
        cfg!(feature = "miniflux"),
    );
    mixed_article(iterations)?;
    raster(iterations)
}

fn mixed_article(iterations: usize) -> Result<()> {
    let source = ARTICLE.repeat(COPIES);
    let formula = if cfg!(feature = "math") {
        "x²"
    } else {
        "$x^2$"
    };
    measure(
        "64 sections: formulas, code, tables and mappings",
        iterations,
        || {
            let rendered = reader::render(black_box(&source), 78);
            verify_article(&source, &rendered, formula)
        },
    )
}

fn verify_article(
    source: &str,
    rendered: &reader::Rendered,
    formula: &str,
) -> Result<()> {
    let index = reader::index::DocumentIndex::parse(source);
    ensure!(index.headings.len() == COPIES, "Headings changed");
    ensure!(index.code.len() == COPIES, "Code index changed");
    ensure!(rendered.table_rows.len() == COPIES, "Tables changed");
    ensure!(
        rendered.text.matches(formula).count() == COPIES * 2,
        "Formula fallback changed"
    );
    ensure!(
        rendered.text.matches("let café = 42;").count() == COPIES,
        "Code contents changed"
    );
    let start = rendered
        .text
        .find(formula)
        .ok_or_else(|| anyhow::anyhow!("Missing formula"))?;
    let original =
        reader::to_source(&rendered.mappings, start..start + formula.len())
            .ok_or_else(|| anyhow::anyhow!("Missing source mapping"))?;
    ensure!(&source[original] == "$x^2$", "Formula source changed");
    verify_syntax(rendered)
}

fn verify_syntax(rendered: &reader::Rendered) -> Result<()> {
    let keyword_color = ratatui::style::Color::Rgb(216, 164, 116);
    let colored_code = rendered
        .lines
        .iter()
        .zip(&rendered.code)
        .filter(|(_, code)| **code)
        .filter(|(line, _)| {
            line.spans.iter().any(|span| {
                span.content.contains("let")
                    && span.style.fg == Some(keyword_color)
            })
        })
        .count();
    ensure!(
        colored_code == if cfg!(feature = "syntax") { COPIES } else { 0 },
        "Syntax capability changed"
    );
    Ok(())
}

fn raster(iterations: usize) -> Result<()> {
    let bytes = raster_input()?;
    let key = media::ImageKey {
        url: "https://example.org/fixture.png".into(),
        width: 64,
        height: 32,
        background: [23, 23, 26],
        detail: None,
    };
    let mut decoder = media::Decoder::default();
    measure(
        "128×64 raster source and portable cells",
        iterations,
        || {
            let prepared = decoder.render(black_box(&bytes), &key)?;
            ensure!(
                prepared.dimensions == (128, 64),
                "Source dimensions changed"
            );
            ensure!(
                prepared.native.is_none() && prepared.rows.is_none(),
                "Portable request produced native pixels"
            );
            ensure!(
                prepared.cells[(0, 0)].fg
                    == ratatui::style::Color::Rgb(255, 0, 0),
                "Top pixels changed"
            );
            ensure!(
                prepared.cells[(0, 15)].bg
                    == ratatui::style::Color::Rgb(0, 0, 255),
                "Bottom pixels changed"
            );
            Ok(())
        },
    )
}

fn raster_input() -> Result<Vec<u8>> {
    let pixels = image::RgbImage::from_fn(128, 64, |_, row| {
        image::Rgb(if row < 32 { [255, 0, 0] } else { [0, 0, 255] })
    });
    let mut bytes = Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(pixels)
        .write_to(&mut bytes, image::ImageFormat::Png)?;
    Ok(bytes.into_inner())
}

fn measure(
    label: &str,
    iterations: usize,
    mut work: impl FnMut() -> Result<()>,
) -> Result<()> {
    let started = Instant::now();
    for _ in 0..iterations {
        work()?;
    }
    println!(
        "{label}: {iterations} checked iterations in {:.3} ms",
        started.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}
