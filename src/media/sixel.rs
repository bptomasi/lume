//! Worker-prepared Sixel strips make arbitrary terminal-row clipping bounded.
//!
//! ratatui-image owns quantization, encoding and fixed-widget placement. Each
//! fitted image is split once at cell-height boundaries so drawing can select
//! complete cached strips in either direction without decoding or resizing.

use anyhow::{Result, ensure};
use image::DynamicImage;
use ratatui::{buffer::Buffer, layout::Rect, widgets::Widget};
use ratatui_image::{
    Image,
    protocol::{Protocol, sixel::Sixel},
};

const MAX_ENCODED_BYTES: usize = 8 * 1024 * 1024;

/// Immutable cell-height Sixel strips, prepared outside the input/draw path.
pub struct SixelRows {
    cells: Buffer,
    bytes: usize,
}

impl SixelRows {
    pub(super) fn new(
        image: DynamicImage,
        area: Rect,
        cell_height: u16,
    ) -> Result<Self> {
        let mut cells = Buffer::empty(area);
        let mut bytes =
            cells.content.len() * std::mem::size_of::<ratatui::buffer::Cell>();
        for row in 0..area.height {
            let y = u32::from(row) * u32::from(cell_height);
            let height = u32::from(cell_height).min(image.height() - y);
            let strip = image.crop_imm(0, y, image.width(), height);
            let target = Rect::new(0, row, area.width, 1);
            let sixel =
                Sixel::new(strip, Rect::new(0, 0, area.width, 1), false)?;
            bytes += sixel.data.len();
            ensure!(bytes <= MAX_ENCODED_BYTES, "Inline pixels exceed 8 MiB");
            Image::new(&Protocol::Sixel(sixel)).render(target, &mut cells);
        }
        Ok(Self { cells, bytes })
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
        if area.is_empty()
            || area.width < self.cells.area.width
            || first_row >= self.cells.area.height
            || area.intersection(buffer.area) != area
        {
            return;
        }
        let height = area.height.min(self.cells.area.height - first_row);
        for row in 0..height {
            for column in 0..self.cells.area.width {
                buffer[(area.x + column, area.y + row)] =
                    self.cells[(column, first_row + row)].clone();
            }
        }
    }
}
