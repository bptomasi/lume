//! Repeated source switches and failed formula projections retain checked output.
use anyhow::{Result, ensure};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use lume::{
    media::{Decoder, ImageKey},
    reader,
};
use ratatui::style::Color;
use std::io::Cursor;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    alternating_rasters(iterations, stress)?;
    alternating_vectors(iterations, stress)?;
    unsupported_formulas(iterations, stress)
}

fn alternating_rasters(iterations: usize, stress: bool) -> Result<()> {
    let width = if stress { 2400 } else { 1200 };
    let mut sources = Vec::new();
    for color in [[255, 0, 0], [0, 0, 255]] {
        let mut bytes = Cursor::new(Vec::new());
        DynamicImage::ImageRgb8(RgbImage::from_pixel(
            width,
            width / 2,
            Rgb(color),
        ))
        .write_to(&mut bytes, ImageFormat::Png)?;
        sources.push((
            bytes.into_inner(),
            Color::Rgb(color[0], color[1], color[2]),
        ));
    }
    let key = key();
    let mut decoder = Decoder::default();
    for (bytes, _) in &sources {
        decoder.render(bytes, &key)?;
    }
    super::measure("alternate two raster sources + fit", iterations, || {
        for (bytes, expected) in &sources {
            let rendered = decoder.render(bytes, &key)?;
            ensure!(
                rendered.dimensions == (width, width / 2),
                "Source dimensions changed"
            );
            ensure!(
                rendered
                    .cells
                    .content
                    .iter()
                    .any(|cell| cell.bg == *expected),
                "Source color changed"
            );
        }
        Ok(())
    })
}

fn alternating_vectors(iterations: usize, stress: bool) -> Result<()> {
    let paths = if stress { 900 } else { 150 };
    let sources = ["#ff0000", "#0000ff"].map(|color| {
        let mut svg = String::from(concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" "#,
            r#"width="1200" height="600">"#,
        ));
        for id in 0..paths {
            svg.push_str(&format!(
                r#"<path fill="{color}" d="M{} {}h35v15h-35z"/>"#,
                id % 30 * 40,
                id / 30 * 20
            ));
        }
        svg.push_str("</svg>");
        svg
    });
    let mut decoder = Decoder::default();
    let key = key();
    for svg in &sources {
        decoder.render(svg.as_bytes(), &key)?;
    }
    super::measure("alternate two SVG sources + fit", iterations, || {
        for svg in &sources {
            let rendered = decoder.render(svg.as_bytes(), &key)?;
            ensure!(
                rendered.dimensions == (1200, 600),
                "Vector dimensions changed"
            );
            ensure!(
                rendered
                    .cells
                    .content
                    .iter()
                    .any(|cell| cell.bg != Color::Rgb(23, 23, 26)),
                "Vector geometry lost"
            );
        }
        Ok(())
    })
}

fn unsupported_formulas(iterations: usize, stress: bool) -> Result<()> {
    let copies = if stress { 120 } else { 20 };
    let expression = r"$\phantom{\frac{a+b+c+d}{\sqrt{x^2+y^2}}}$";
    let body = format!("{expression}\n\n").repeat(copies);
    super::measure(
        "repeat unsupported formula with exact source fallback",
        iterations,
        || {
            let rendered = reader::render(&body, 80);
            ensure!(
                rendered.text.matches(expression).count() == copies,
                "Raw formula fallback changed"
            );
            Ok(())
        },
    )
}

fn key() -> ImageKey {
    ImageKey {
        url: "https://benchmark.example/source".into(),
        width: 80,
        height: 12,
        background: [23, 23, 26],
        detail: None,
    }
}
