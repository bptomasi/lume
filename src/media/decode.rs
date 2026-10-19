//! Qualified decoders own formats; local budgets precede pixel allocation.

use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, Rgba, RgbaImage};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_image::{
    Image,
    protocol::{Protocol, halfblocks::Halfblocks},
};

use super::{ImageKey, MAX_BYTES, Thumbnail};

/// Reuses the SVG font database across images; used only outside UI drawing.
#[derive(Default)]
pub struct Decoder {
    svg: super::svg::SvgDecoder,
}

impl Decoder {
    /// Decode one bounded image and prepare an aspect-preserving cell buffer.
    ///
    /// # Errors
    /// Returns unsupported, malformed or oversized content and encoding errors.
    pub fn render(
        &mut self,
        bytes: &[u8],
        key: &ImageKey,
    ) -> Result<Thumbnail> {
        ensure!(bytes.len() <= MAX_BYTES, "Image exceeds 8 MiB");
        ensure!(
            (1..=160).contains(&key.width) && (1..=48).contains(&key.height),
            "Image viewport exceeds 160 columns or 48 rows"
        );
        let (image, dimensions) = if image::guess_format(bytes).is_ok() {
            raster(bytes, key)?
        } else {
            self.svg.render(bytes, key)?
        };
        let width = image.width() as u16;
        let height = image.height().div_ceil(2) as u16;
        let image = composite(image, key.background);
        let area = Rect::new(0, 0, width, height);
        let protocol = Protocol::Halfblocks(Halfblocks::new(image, area)?);
        let mut cells = Buffer::empty(area);
        Image::new(&protocol).render(area, &mut cells);
        // Keep every raster cell identifiable as literal image color. A full
        // halfblock with identical colors is visually equal to a colored space.
        for cell in &mut cells.content {
            if cell.symbol() == " " {
                cell.set_symbol("▀");
            }
        }
        Ok(Thumbnail {
            cells,
            dimensions,
            cache_warning: None,
        })
    }
}

fn raster(bytes: &[u8], key: &ImageKey) -> Result<(DynamicImage, (u32, u32))> {
    let dimensions = reader(bytes)?.into_dimensions()?;
    ensure!(
        u64::from(dimensions.0) * u64::from(dimensions.1) <= 8_000_000,
        "Image exceeds 8 million pixels"
    );
    let image = reader(bytes)?.decode().context("Invalid image data")?;
    let thumbnail = image.resize(
        u32::from(key.width),
        u32::from(key.height) * 2,
        image::imageops::FilterType::Triangle,
    );
    Ok((thumbnail, dimensions))
}

fn reader(bytes: &[u8]) -> Result<ImageReader<Cursor<&[u8]>>> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .context("Could not inspect image format")?;
    ensure!(
        matches!(
            reader.format(),
            Some(
                ImageFormat::Png
                    | ImageFormat::Jpeg
                    | ImageFormat::WebP
                    | ImageFormat::Gif
            )
        ),
        "Supported images: PNG, JPEG, WebP, GIF and SVG"
    );
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    Ok(reader)
}

fn composite(image: DynamicImage, background: [u8; 3]) -> DynamicImage {
    let [red, green, blue] = background;
    let mut canvas = RgbaImage::from_pixel(
        image.width(),
        image.height().next_multiple_of(2),
        Rgba([red, green, blue, 255]),
    );
    image::imageops::overlay(&mut canvas, &image.to_rgba8(), 0, 0);
    DynamicImage::ImageRgba8(canvas)
}
