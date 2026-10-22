//! Worker-prepared Sixel strips make arbitrary terminal-row clipping bounded.
//!
//! Quantette owns shared-palette quantization; icy_sixel owns encoding and
//! ratatui-image owns fixed-widget placement. Each
//! fitted image is split once at cell-height boundaries so drawing can select
//! complete cached strips in either direction without decoding or resizing.

use anyhow::{Result, ensure};
use icy_sixel::{PixelFormat, dither::sixel_dither, output::sixel_output};
use image::DynamicImage;
use quantette::{ImageRef, Pipeline, deps::palette::Srgb};
use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    widgets::Widget,
};
use ratatui_image::{
    Image,
    protocol::{Protocol, sixel::Sixel},
};

const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;

/// Immutable cell-height Sixel strips, prepared outside the input/draw path.
///
/// Only the payload cell is retained per row. Drawing marks the remaining
/// cells as covered instead of storing and cloning an empty image-sized grid.
pub struct SixelRows {
    area: Rect,
    rows: Vec<Cell>,
    bytes: usize,
}

impl SixelRows {
    pub(super) fn new(
        image: DynamicImage,
        area: Rect,
        cell_height: u16,
    ) -> Result<Self> {
        let rgb = image.to_rgb8();
        let (mut palette, indexed) = indexed_pixels(&rgb)?;
        palette.set_pixelformat(PixelFormat::PAL8);
        let strip_area = Rect::new(0, 0, area.width, 1);
        let mut scratch = Buffer::empty(strip_area);
        let mut rows = Vec::with_capacity(usize::from(area.height));
        let mut bytes = rows.capacity() * std::mem::size_of::<Cell>();
        for row in 0..area.height {
            let y = u32::from(row) * u32::from(cell_height);
            let height = u32::from(cell_height).min(image.height() - y);
            let start = (y * image.width()) as usize;
            let end = start + (height * image.width()) as usize;
            let mut strip = indexed[start..end].to_vec();
            let sixel = encode(
                &mut strip,
                image.width(),
                height,
                strip_area,
                &mut palette,
            )?;
            bytes += sixel.data.len();
            ensure!(bytes <= MAX_ENCODED_BYTES, "Inline pixels exceed 8 MiB");
            Image::new(&Protocol::Sixel(sixel))
                .render(strip_area, &mut scratch);
            rows.push(std::mem::take(&mut scratch[(0, 0)]));
        }
        Ok(Self { area, rows, bytes })
    }

    /// Retained encoded bytes and cells, counted by the shared image cache.
    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Copy only complete visible rows, after theming and transient overlays.
    ///
    /// Narrow, empty and out-of-bounds targets draw nothing. The caller owns
    /// centering and synchronized presentation; this method performs no I/O.
    pub fn render(&self, area: Rect, first_row: u16, buffer: &mut Buffer) {
        self.render_uncovered(area, first_row, buffer, &[]);
    }

    /// Paint cached strips outside popup footprints; partial strips stay hidden.
    /// A Sixel strip spans the fitted image width and cannot be cropped sideways.
    pub fn render_uncovered(
        &self,
        area: Rect,
        first_row: u16,
        buffer: &mut Buffer,
        occlusions: &[Rect],
    ) {
        if area.is_empty()
            || area.width < self.area.width
            || first_row >= self.area.height
            || area.intersection(buffer.area) != area
        {
            return;
        }
        let height = area.height.min(self.area.height - first_row);
        for row in 0..height {
            let strip = Rect::new(area.x, area.y + row, self.area.width, 1);
            if occlusions
                .iter()
                .any(|popup| !strip.intersection(*popup).is_empty())
            {
                continue;
            }
            buffer[(area.x, area.y + row)] =
                self.rows[usize::from(first_row + row)].clone();
            for column in 1..self.area.width {
                buffer[(area.x + column, area.y + row)].set_skip(true);
            }
        }
    }
}

fn indexed_pixels(image: &image::RgbImage) -> Result<(sixel_dither, Vec<u8>)> {
    let pixels: Vec<_> = image
        .pixels()
        .map(|pixel| Srgb::new(pixel[0], pixel[1], pixel[2]))
        .collect();
    let input = ImageRef::new(image.width(), image.height(), &pixels)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    // Quantize and diffuse once, including the boundaries between strips.
    let indexed = Pipeline::new()
        .input_image(input)
        .output_srgb8_indexed_image();
    let mut palette = sixel_dither::new(indexed.palette().len() as i32)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    palette.set_palette(
        indexed
            .palette()
            .iter()
            .flat_map(|color| [color.red, color.green, color.blue])
            .collect(),
    );
    palette.set_optimize_palette(false);
    Ok((palette, indexed.indices().to_vec()))
}

/// The inspector uses the same color preparation as clipped reader images.
pub(super) fn native(image: DynamicImage, area: Rect) -> Result<Protocol> {
    let rgb = image.to_rgb8();
    let (mut palette, mut indexed) = indexed_pixels(&rgb)?;
    palette.set_pixelformat(PixelFormat::PAL8);
    let sixel = encode(
        &mut indexed,
        image.width(),
        image.height(),
        area,
        &mut palette,
    )?;
    ensure!(
        sixel.data.len() <= MAX_ENCODED_BYTES,
        "Native pixels exceed 8 MiB"
    );
    Ok(Protocol::Sixel(sixel))
}

fn encode(
    pixels: &mut [u8],
    width: u32,
    height: u32,
    area: Rect,
    palette: &mut sixel_dither,
) -> Result<Sixel> {
    let mut bytes = Vec::new();
    sixel_output::new(&mut bytes)
        .encode(pixels, width as i32, height as i32, 0, palette)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(Sixel {
        data: String::from_utf8(bytes)?,
        area,
        is_tmux: false,
    })
}
