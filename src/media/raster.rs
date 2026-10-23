//! Two decoded sources share a 32 MiB budget across pan, zoom and image switches.
//! New decoded bytes are admitted before allocation; oversized sources are used
//! once without retention. Failed content never returns another source's pixels.

use super::{ImageKey, geometry};
use anyhow::{Context, Result, ensure};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use sha2::{Digest, Sha256};
use std::{collections::VecDeque, io::Cursor};

const SOURCE_BUDGET: usize = 32 * 1024 * 1024;

struct Source {
    digest: [u8; 32],
    image: DynamicImage,
    opaque: bool,
}

#[derive(Default)]
pub(super) struct RasterDecoder {
    cached: VecDeque<Source>,
}

impl RasterDecoder {
    pub(super) fn render(
        &mut self,
        bytes: &[u8],
        key: &ImageKey,
    ) -> Result<(DynamicImage, (u32, u32))> {
        let digest = Sha256::digest(bytes).into();
        if let Some(index) = self
            .cached
            .iter()
            .position(|source| source.digest == digest)
        {
            let source = self
                .cached
                .remove(index)
                .context("Raster cache entry disappeared")?;
            self.cached.push_front(source);
            let source =
                self.cached.front().context("Raster cache is empty")?;
            return Ok(prepare(source, key));
        }
        let source = self.decode(bytes, digest)?;
        let prepared = prepare(&source, key);
        if source.image.as_bytes().len() <= SOURCE_BUDGET {
            self.cached.push_front(source);
        }
        Ok(prepared)
    }

    fn decode(&mut self, bytes: &[u8], digest: [u8; 32]) -> Result<Source> {
        let mut decoder = reader(bytes)?.into_decoder()?;
        let (width, height) = decoder.dimensions();
        ensure!(width > 0 && height > 0, "Image has no pixels");
        ensure!(
            u64::from(width) * u64::from(height) <= 8_388_608,
            "Image exceeds 8,388,608 pixels"
        );
        let mut limits = limits();
        limits.reserve(decoder.total_bytes())?;
        decoder.set_limits(limits)?;
        self.reserve(decoder.total_bytes());
        let orientation = decoder.orientation()?;
        let mut image = DynamicImage::from_decoder(decoder)
            .context("Invalid image data")?;
        image.apply_orientation(orientation);
        let opaque = !image.color().has_alpha() || opaque_alpha(&image);
        Ok(Source {
            digest,
            image,
            opaque,
        })
    }

    fn reserve(&mut self, incoming: u64) {
        while self.cached.len() >= 2
            || self
                .cached
                .iter()
                .map(|source| source.image.as_bytes().len() as u64)
                .sum::<u64>()
                .saturating_add(incoming)
                > SOURCE_BUDGET as u64
        {
            if self.cached.pop_back().is_none() {
                break;
            }
        }
    }
}

fn prepare(source: &Source, key: &ImageKey) -> (DynamicImage, (u32, u32)) {
    let image = &source.image;
    let prepared = if key.detail.is_some() {
        crop_raster(source, key)
    } else {
        let image = matte(image, key, source.opaque);
        image.resize(
            u32::from(key.width),
            u32::from(key.height) * 2,
            image::imageops::FilterType::Triangle,
        )
    };
    (prepared, (image.width(), image.height()))
}

fn crop_raster(source: &Source, key: &ImageKey) -> DynamicImage {
    let image = &source.image;
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
    let crop = image.crop_imm(
        x,
        y,
        width.max(1).min(image.width() - x),
        height.max(1).min(image.height() - y),
    );
    matte(&crop, key, source.opaque).resize_exact(
        geometry.width,
        geometry.height,
        image::imageops::FilterType::Lanczos3,
    )
}

fn matte<'a>(
    image: &'a DynamicImage,
    key: &ImageKey,
    opaque: bool,
) -> std::borrow::Cow<'a, DynamicImage> {
    if opaque {
        return std::borrow::Cow::Borrowed(image);
    }
    if matches!(
        image.color(),
        image::ColorType::Rgba16 | image::ColorType::La16
    ) {
        return std::borrow::Cow::Owned(matte16(image, key));
    }
    // Hidden RGB must never bleed into resized edges. Composite against the
    // actual matte before filtering, retaining the original cached source.
    let mut canvas = image::RgbaImage::from_pixel(
        image.width(),
        image.height(),
        image::Rgba([
            key.background[0],
            key.background[1],
            key.background[2],
            255,
        ]),
    );
    let pixels = image
        .as_rgba8()
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| std::borrow::Cow::Owned(image.to_rgba8()));
    image::imageops::overlay(&mut canvas, pixels.as_ref(), 0, 0);
    std::borrow::Cow::Owned(DynamicImage::ImageRgba8(canvas))
}

fn opaque_alpha(image: &DynamicImage) -> bool {
    match image {
        DynamicImage::ImageRgba8(pixels) => {
            pixels.pixels().all(|p| p[3] == u8::MAX)
        }
        DynamicImage::ImageRgba16(pixels) => {
            pixels.pixels().all(|p| p[3] == u16::MAX)
        }
        DynamicImage::ImageLumaA8(pixels) => {
            pixels.pixels().all(|p| p[1] == u8::MAX)
        }
        DynamicImage::ImageLumaA16(pixels) => {
            pixels.pixels().all(|p| p[1] == u16::MAX)
        }
        _ => false,
    }
}

fn matte16(image: &DynamicImage, key: &ImageKey) -> DynamicImage {
    let [red, green, blue] =
        key.background.map(|channel| u16::from(channel) * 257);
    let mut canvas = image::ImageBuffer::from_pixel(
        image.width(),
        image.height(),
        image::Rgba([red, green, blue, u16::MAX]),
    );
    let pixels = image
        .as_rgba16()
        .map(std::borrow::Cow::Borrowed)
        .unwrap_or_else(|| std::borrow::Cow::Owned(image.to_rgba16()));
    image::imageops::overlay(&mut canvas, pixels.as_ref(), 0, 0);
    DynamicImage::ImageRgba16(canvas)
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
    reader.limits(limits());
    Ok(reader)
}

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(8192);
    limits.max_image_height = Some(8192);
    limits.max_alloc = Some(64 * 1024 * 1024);
    limits
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(digest: u8) -> Source {
        Source {
            digest: [digest; 32],
            opaque: true,
            image: DynamicImage::new_rgb8(1024, 1024),
        }
    }

    #[test]
    fn admissions_evict_before_decode_and_never_exceed_the_shared_budget() {
        let mut cache = RasterDecoder::default();
        cache.cached.push_front(source(1));
        cache.cached.push_front(source(2));
        cache.reserve(3 * 1024 * 1024);
        assert_eq!(cache.cached.len(), 1);
        assert_eq!(
            cache.cached.front().map(|source| source.digest),
            Some([2; 32])
        );
        cache.reserve(SOURCE_BUDGET as u64);
        assert!(cache.cached.is_empty());
        cache.cached.push_front(source(1));
        cache.reserve(SOURCE_BUDGET as u64 + 1);
        assert!(cache.cached.is_empty());
    }
}
