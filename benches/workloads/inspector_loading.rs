//! Full-resolution inspector preparation, including lossless and photographic color.
use anyhow::{Context, Result, ensure};
use image::{DynamicImage, ImageFormat, Rgb, RgbImage};
use lume::media::{Decoder, ImageDetail, ImageKey, ImageProtocol};
use std::io::Cursor;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let width = if stress { 160 } else { 120 };
    let key = ImageKey {
        url: "https://benchmark.example/inspector.png".into(),
        width,
        height: (width / 3).min(48),
        background: [23, 23, 26],
        detail: Some(ImageDetail {
            zoom: 1,
            pan: [0, 0],
            protocol: ImageProtocol::Sixel,
            cell_pixels: (10, 20),
            slice: None,
        }),
    };
    for (name, bytes) in fixtures(stress)? {
        let mut decoder = Decoder::default();
        decoder.render(&bytes, &key)?;
        super::measure(name, iterations.min(4), || {
            let image = decoder.render(&bytes, &key)?;
            ensure!(image.dimensions == (1200, 600), "Source changed");
            ensure!(image.native.is_some(), "Native inspector disappeared");
            ensure!(image.cells.area.width == width, "Resolution changed");
            Ok(())
        })?;
    }
    Ok(())
}

fn fixtures(stress: bool) -> Result<Vec<(&'static str, Vec<u8>)>> {
    let gradient = RgbImage::from_fn(1200, 600, |x, y| {
        Rgb([
            (x * 255 / 1199) as u8,
            (y * 255 / 599) as u8,
            ((x + y) * 255 / 1798) as u8,
        ])
    });
    let mut bytes = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(gradient).write_to(&mut bytes, ImageFormat::Png)?;
    let noise = RgbImage::from_fn(1200, 600, |x, y| {
        let seed = (x + y * 1200).wrapping_mul(2_654_435_761);
        Rgb([(seed >> 24) as u8, (seed >> 16) as u8, (seed >> 8) as u8])
    });
    let mut detail = Cursor::new(Vec::new());
    DynamicImage::ImageRgb8(noise).write_to(&mut detail, ImageFormat::Png)?;
    // Keep source dimensions stable; stress expands the output pixel budget.
    let diagram = super::images::raster(false).context("Diagram fixture")?;
    Ok(vec![
        ("inspector exact-color raster + Sixel", diagram),
        (
            "inspector SVG text + Sixel",
            super::images::vector(stress).into_bytes(),
        ),
        (
            "inspector photographic gradient + Sixel",
            bytes.into_inner(),
        ),
        ("inspector high-entropy detail + Sixel", detail.into_inner()),
    ])
}
