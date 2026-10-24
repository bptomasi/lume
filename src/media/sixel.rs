//! Worker-prepared Sixel strips make arbitrary terminal-row clipping bounded.
//!
//! Quantette owns shared-palette quantization; icy_sixel owns encoding and
//! ratatui-image owns fixed-widget placement. Each
//! fitted image is split once at cell-height boundaries so drawing can select
//! complete cached strips in either direction without decoding or resizing.
//! Each strip erases its full cell width before painting pixels: skipped cells
//! can still hold text from an earlier frame, including partially covered edges.

use anyhow::{Result, ensure};
use image::DynamicImage;
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
mod encoding;
mod palette;

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
        let (palette, indexed) = palette::index(&rgb)?;
        let strip_area = Rect::new(0, 0, area.width, 1);
        let mut scratch = Buffer::empty(strip_area);
        let mut rows = Vec::with_capacity(usize::from(area.height));
        let mut bytes = rows.capacity() * std::mem::size_of::<Cell>();
        for row in 0..area.height {
            let y = u32::from(row) * u32::from(cell_height);
            let height = u32::from(cell_height).min(image.height() - y);
            let start = (y * image.width()) as usize;
            let end = start + (height * image.width()) as usize;
            let strip = &indexed[start..end];
            let sixel =
                encode(strip, image.width(), height, strip_area, &palette)?;
            Image::new(&Protocol::Sixel(sixel))
                .render(strip_area, &mut scratch);
            let mut cell = std::mem::take(&mut scratch[(0, 0)]);
            // ECH clears text without moving the cursor or crossing this row.
            cell.set_symbol(&format!("\x1b[{}X{}", area.width, cell.symbol()));
            bytes += cell.symbol().len();
            ensure!(bytes <= MAX_ENCODED_BYTES, "Inline pixels exceed 8 MiB");
            rows.push(cell);
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
            let style = buffer[(area.x, area.y + row)].style();
            buffer[(area.x, area.y + row)] =
                self.rows[usize::from(first_row + row)].clone();
            buffer[(area.x, area.y + row)].set_style(style);
            for column in 1..self.area.width {
                buffer[(area.x + column, area.y + row)].set_skip(true);
            }
        }
    }
}

/// The inspector uses the same color preparation as clipped reader images.
pub(super) fn native(image: DynamicImage, area: Rect) -> Result<Protocol> {
    let rgb = image.to_rgb8();
    let (palette, indexed) = palette::index(&rgb)?;
    let sixel =
        encode(&indexed, image.width(), image.height(), area, &palette)?;
    ensure!(
        sixel.data.len() <= MAX_ENCODED_BYTES,
        "Native pixels exceed 8 MiB"
    );
    Ok(Protocol::Sixel(sixel))
}

fn encode(
    pixels: &[u8],
    width: u32,
    height: u32,
    area: Rect,
    palette: &[u8],
) -> Result<Sixel> {
    let mut bytes = Vec::new();
    encoding::write(&mut bytes, pixels, width, height, palette)
        .map_err(|error| anyhow::anyhow!("{error}"))?;
    Ok(Sixel {
        data: String::from_utf8(bytes)?,
        area,
        is_tmux: false,
    })
}
