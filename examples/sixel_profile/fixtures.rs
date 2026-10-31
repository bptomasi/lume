//! Deterministic, separately named diagram, photographic and synthetic inputs.
use anyhow::{Result, bail};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage, Rgba, RgbaImage};
use std::io::Cursor;

pub(super) fn source(name: &str) -> Result<Vec<u8>> {
    match name {
        "diagram" => Ok(include_bytes!("../../tests/fixtures/images/request-path.svg").to_vec()),
        "photo" => Ok(include_bytes!("../../tests/fixtures/images/astronaut.png").to_vec()),
        "gradient" => png(DynamicImage::ImageRgb8(RgbImage::from_fn(1200, 600, |x, y| {
            Rgb([(x * 255 / 1199) as u8, (y * 255 / 599) as u8,
                ((x + y) * 255 / 1798) as u8])
        }))),
        "entropy" => png(DynamicImage::ImageRgb8(RgbImage::from_fn(1200, 600, |x, y| {
            let mut n = (y * 1200 + x + 1).wrapping_mul(0x9e37_79b9);
            n ^= n >> 16;
            n = n.wrapping_mul(0x85eb_ca6b);
            n ^= n >> 13;
            Rgb([n as u8, (n >> 8) as u8, (n >> 16) as u8])
        }))),
        "transparent" => png(DynamicImage::ImageRgba8(RgbaImage::from_fn(1200, 600, |x, y| {
            let alpha = if x < 100 { 0 } else { (y * 255 / 599) as u8 };
            Rgba([(x * 255 / 1199) as u8, 80, 220, alpha])
        }))),
        "antialias" => Ok(concat!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="600">"#,
            r#"<rect width="1200" height="600" fill="white"/>"#,
            r#"<path d="M0 0L1200 600M0 599L1200 0M0 330Q400 30 1200 280" "#,
            r#"stroke="black" fill="none" stroke-width="1.2"/>"#,
            r#"<circle cx="600" cy="300" r="245.7" stroke="black" "#,
            r#"stroke-width="1.1" fill="none"/></svg>"#
        ).as_bytes().to_vec()),
        _ => bail!("Unknown fixture: {name}"),
    }
}

fn png(image: DynamicImage) -> Result<Vec<u8>> {
    let mut output = Cursor::new(Vec::new());
    image.write_to(&mut output, ImageFormat::Png)?;
    Ok(output.into_inner())
}
