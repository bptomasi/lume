//! One decoded raster is retained for pan, zoom and resize of the same bytes.
//!
//! At most 32 MiB are retained, without reducing the source's color precision.
//! Larger sources are prepared and discarded. Replacement drops the old source
//! before decoding; failed input is not cached.

use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};

use super::{ImageKey, geometry};

#[derive(Default)]
pub(super) struct RasterDecoder {
    cached: Option<([u8; 32], DynamicImage)>,
}

impl RasterDecoder {
    pub(super) fn clear(&mut self) {
        self.cached = None;
    }

    pub(super) fn render(
        &mut self,
        bytes: &[u8],
        key: &ImageKey,
    ) -> Result<(DynamicImage, (u32, u32))> {
        let digest: [u8; 32] = Sha256::digest(bytes).into();
        if let Some((old, image)) = &self.cached
            && *old == digest
        {
            return Ok(prepare(image, key));
        }
        self.clear();
        let image = decode(bytes)?;
        let prepared = prepare(&image, key);
        if image.as_bytes().len() <= 32 * 1024 * 1024 {
            self.cached = Some((digest, image));
        }
        Ok(prepared)
    }
}

fn decode(bytes: &[u8]) -> Result<DynamicImage> {
    let dimensions = reader(bytes)?.into_dimensions()?;
    ensure!(dimensions.0 > 0 && dimensions.1 > 0, "Image has no pixels");
    ensure!(
        u64::from(dimensions.0) * u64::from(dimensions.1) <= 8_000_000,
        "Image exceeds 8 million pixels"
    );
    reader(bytes)?.decode().context("Invalid image data")
}

fn prepare(image: &DynamicImage, key: &ImageKey) -> (DynamicImage, (u32, u32)) {
    let prepared = if key.detail.is_some() {
        crop_raster(image, key)
    } else {
        image.resize(
            u32::from(key.width),
            u32::from(key.height) * 2,
            image::imageops::FilterType::Triangle,
        )
    };
    (prepared, (image.width(), image.height()))
}

fn crop_raster(image: &DynamicImage, key: &ImageKey) -> DynamicImage {
    let geometry = geometry::Geometry::new(
        (image.width() as f32, image.height() as f32),
        key,
    );
    let x = (geometry.offset[0] / geometry.scale).floor() as u32;
    let y = (geometry.offset[1] / geometry.scale).floor() as u32;
    let width = (geometry.width as f32 / geometry.scale).ceil() as u32;
    let height = (geometry.height as f32 / geometry.scale).ceil() as u32;
    let x = x.min(image.width() - 1);
    let y = y.min(image.height() - 1);
    // Crop before resizing: zoom cannot allocate an entire enlarged source.
    image
        .crop_imm(
            x,
            y,
            width.max(1).min(image.width() - x),
            height.max(1).min(image.height() - y),
        )
        .resize_exact(
            geometry.width,
            geometry.height,
            image::imageops::FilterType::Lanczos3,
        )
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
