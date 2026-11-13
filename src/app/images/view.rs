//! Borrowed prepared images and exact keys, without worker admission authority.

use super::{App, ReaderImages, viewport::Viewport};
use crate::{
    app::{Focus, Mode, WebPage},
    media::{ImageDetail, ImageKey, ImageProtocol, ImageSlice},
    reader::images::ImageSlot,
};
use ratatui::layout::Rect;

#[derive(Clone, Copy)]
pub(crate) struct ImageView<'a> {
    pub images: &'a ReaderImages,
    pub viewport: Viewport<'a>,
    pub mode: &'a Mode,
    pub focus: Focus,
    pub web_page: Option<&'a WebPage>,
    pub web_active: bool,
    pub cursor: usize,
    pub theme: &'a str,
    screen: Rect,
}

impl ImageView<'_> {
    /// Exact preparation key for an already resolved publisher image slot.
    pub fn image_key(&self, slot: &ImageSlot) -> Option<ImageKey> {
        if slot.url.is_empty()
            || slot.formula.is_some() && !self.uses_sixel_images()
        {
            return None;
        }
        Some(ImageKey {
            url: slot.url.clone(),
            width: self.viewport.width.clamp(1, 160) as u16,
            height: slot.rows.len().clamp(1, 48) as u16,
            background: crate::presentation::palette::image_background(
                self.theme,
            ),
            detail: self.inline_pixel_detail(slot),
        })
    }

    fn inline_pixel_detail(&self, slot: &ImageSlot) -> Option<ImageDetail> {
        if self.images.protocol != ImageProtocol::Sixel {
            return None;
        }
        let viewport = self.viewport;
        let bottom = viewport.scroll + viewport.height;
        let eligible = slot.formula.is_some()
            || viewport
                .visible()
                .iter()
                .filter(|image| {
                    image.formula.is_none() && image.rows.start < bottom
                })
                .take(4)
                .any(|image| image.source == slot.source);
        if !eligible {
            return None;
        }
        Some(ImageDetail {
            zoom: 1,
            pan: [0, 0],
            protocol: self.images.protocol,
            cell_pixels: self.images.cell_pixels,
            slice: Some(ImageSlice {
                first_row: 0,
                rows: slot.rows.len().clamp(1, 48) as u16,
            }),
        })
    }

    /// Exact inspector request, including source crop and terminal cell size.
    pub fn image_inspector_key(&self) -> Option<ImageKey> {
        let Mode::Image(image) = self.mode else {
            return None;
        };
        let area =
            crate::presentation::geometry::image_inspector_area(self.screen);
        let mut width = area.width.max(1);
        let height = area.height.max(1);
        let cell = self.images.cell_pixels;
        if self.images.protocol != ImageProtocol::Blocks {
            let column_pixels = u64::from(height)
                * u64::from(cell.0.max(1))
                * u64::from(cell.1.max(1));
            width =
                (4_000_000 / column_pixels).clamp(1, u64::from(width)) as u16;
        }
        Some(ImageKey {
            url: image.url.clone(),
            width,
            height,
            background: crate::presentation::palette::image_background(
                self.theme,
            ),
            detail: Some(ImageDetail {
                zoom: image.zoom,
                pan: image.pan,
                protocol: self.images.protocol,
                cell_pixels: cell,
                slice: None,
            }),
        })
    }

    pub(crate) fn uses_sixel_images(&self) -> bool {
        self.images.protocol == ImageProtocol::Sixel
    }
}

impl App {
    pub(crate) fn image_view(&self) -> ImageView<'_> {
        ImageView {
            images: &self.images,
            viewport: self.image_viewport(),
            mode: self.mode(),
            focus: self.focus(),
            web_page: self.web_page(),
            web_active: self.web_active(),
            cursor: self.reader().reader_cursor,
            theme: &self.workspace().theme,
            screen: self.layout().screen,
        }
    }
}
