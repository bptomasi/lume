//! Qualified decoders own formats; local budgets precede pixel allocation.

use std::io::Cursor;

use anyhow::{Context, Result, ensure};
use image::{DynamicImage, ImageFormat, ImageReader, Limits, Rgba, RgbaImage};
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_image::{
    Image, Resize,
    picker::{Picker, ProtocolType},
    protocol::{Protocol, halfblocks::Halfblocks},
};

use super::{ImageKey, ImageProtocol, MAX_BYTES, Thumbnail, geometry};

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
        geometry::validate(key)?;
        let (image, dimensions) = if image::guess_format(bytes).is_ok() {
            raster(bytes, key)?
        } else {
            self.svg.render(bytes, key)?
        };
        prepare(image, dimensions, key)
    }
}

fn prepare(
    image: DynamicImage,
    dimensions: (u32, u32),
    key: &ImageKey,
) -> Result<Thumbnail> {
    let image = slice(image, key);
    let (cell_width, cell_height) = geometry::cell_pixels(key);
    let width = image.width().div_ceil(u32::from(cell_width)) as u16;
    let height = image.height().div_ceil(u32::from(cell_height)) as u16;
    let image = composite(image, key);
    let area = Rect::new(0, 0, width, height);
    if key.detail.as_ref().is_some_and(|detail| {
        detail.protocol == ImageProtocol::Sixel && detail.slice.is_some()
    }) {
        return inline_thumbnail(image, area, dimensions, cell_height);
    }
    let protocol = encode(image, area, key)?;
    if !matches!(protocol, Protocol::Halfblocks(_)) {
        return Ok(Thumbnail {
            cells: Buffer::empty(protocol.area()),
            dimensions,
            cache_warning: None,
            native: Some(protocol),
            rows: None,
        });
    }
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
        native: None,
        rows: None,
    })
}

fn inline_thumbnail(
    image: DynamicImage,
    area: Rect,
    dimensions: (u32, u32),
    cell_height: u16,
) -> Result<Thumbnail> {
    Ok(Thumbnail {
        cells: Buffer::empty(area),
        dimensions,
        cache_warning: None,
        native: None,
        rows: Some(super::SixelRows::new(image, area, cell_height)?),
    })
}

fn encode(image: DynamicImage, area: Rect, key: &ImageKey) -> Result<Protocol> {
    let Some(detail) = &key.detail else {
        return Ok(Protocol::Halfblocks(Halfblocks::new(image, area)?));
    };
    let protocol = match detail.protocol {
        ImageProtocol::Blocks => {
            return Ok(Protocol::Halfblocks(Halfblocks::new(image, area)?));
        }
        ImageProtocol::Sixel => ProtocolType::Sixel,
        ImageProtocol::Iterm2 => ProtocolType::Iterm2,
    };
    let mut picker = Picker::from_fontsize(detail.cell_pixels);
    picker.set_protocol_type(protocol);
    picker.set_background_color(Rgba([
        key.background[0],
        key.background[1],
        key.background[2],
        255,
    ]));
    Ok(picker.new_protocol(image, area, Resize::Fit(None))?)
}

fn slice(image: DynamicImage, key: &ImageKey) -> DynamicImage {
    let Some(slice) =
        key.detail.as_ref().and_then(|detail| detail.slice.as_ref())
    else {
        return image;
    };
    let (_, cell_height) = geometry::cell_pixels(key);
    let start = (u32::from(slice.first_row) * u32::from(cell_height))
        .min(image.height());
    let height = (u32::from(slice.rows) * u32::from(cell_height))
        .min(image.height() - start);
    image.crop_imm(0, start, image.width(), height)
}

fn raster(bytes: &[u8], key: &ImageKey) -> Result<(DynamicImage, (u32, u32))> {
    let dimensions = reader(bytes)?.into_dimensions()?;
    ensure!(dimensions.0 > 0 && dimensions.1 > 0, "Image has no pixels");
    ensure!(
        u64::from(dimensions.0) * u64::from(dimensions.1) <= 8_000_000,
        "Image exceeds 8 million pixels"
    );
    let image = reader(bytes)?.decode().context("Invalid image data")?;
    if key.detail.is_some() {
        return Ok((crop_raster(image, key), dimensions));
    }
    let thumbnail = image.resize(
        u32::from(key.width),
        u32::from(key.height) * 2,
        image::imageops::FilterType::Triangle,
    );
    Ok((thumbnail, dimensions))
}

fn crop_raster(image: DynamicImage, key: &ImageKey) -> DynamicImage {
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

fn composite(image: DynamicImage, key: &ImageKey) -> DynamicImage {
    let [red, green, blue] = key.background;
    let height = if key
        .detail
        .as_ref()
        .is_none_or(|detail| detail.protocol == ImageProtocol::Blocks)
    {
        image.height().next_multiple_of(2)
    } else {
        // Native pixels must fit the exact cell budget. Padding an odd final
        // row could force the terminal encoder to rescale the fitted image.
        image.height()
    };
    let mut canvas = RgbaImage::from_pixel(
        image.width(),
        height,
        Rgba([red, green, blue, 255]),
    );
    image::imageops::overlay(&mut canvas, &image.to_rgba8(), 0, 0);
    DynamicImage::ImageRgba8(canvas)
}
