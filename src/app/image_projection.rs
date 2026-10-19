//! Only unobscured reading surfaces may own external image windows.

use ratatui::layout::Rect;

use crate::{media::ImageKey, overlay::Placement, ui};

use super::{App, Mode};

impl App {
    /// Current clipped PNG projections; overlays never cover menus or pane drags.
    pub fn overlay_placements(&self) -> Vec<Placement> {
        if !self.uses_image_overlay()
            || self.pane_motion_active()
            || self.hovered_note().is_some()
        {
            return Vec::new();
        }
        match &self.mode {
            Mode::Image(_) => self
                .image_inspector_key()
                .and_then(|key| {
                    self.projected_image(
                        "lume-inspector".into(),
                        &key,
                        ui::image_inspector_area(self.screen),
                        true,
                    )
                })
                .into_iter()
                .collect(),
            Mode::Normal
                if self.notebook.is_none() && self.selection.is_none() =>
            {
                self.reader_projections()
            }
            _ => Vec::new(),
        }
    }

    fn reader_projections(&self) -> Vec<Placement> {
        let Some(reader) =
            ui::workspace_areas(self.screen, self.focus, &self.workspace)
                .reader
        else {
            return Vec::new();
        };
        let column = ui::workspace_reader_content(reader, &self.workspace);
        self.images
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| {
                self.reader_projection(index, slot, column)
            })
            .collect()
    }

    fn reader_projection(
        &self,
        index: usize,
        slot: &crate::reader::images::ImageSlot,
        column: Rect,
    ) -> Option<Placement> {
        let key = self.image_key(slot)?;
        let detail = key.detail.as_ref()?;
        let image = self.images.thumbnail(&key)?;
        let first = self.scroll.saturating_sub(slot.rows.start);
        let bottom = self.scroll + usize::from(column.height);
        let last = bottom
            .saturating_sub(slot.rows.start)
            .min(usize::from(image.cells.area.height));
        if first >= last || column.is_empty() {
            return None;
        }
        let y = slot.rows.start.saturating_sub(self.scroll) as u16;
        let area = Rect::new(
            column.x,
            column.y + y,
            column.width,
            (last - first) as u16,
        );
        let mut placement = self.projected_image(
            format!("lume-image-{index}"),
            &key,
            area,
            false,
        )?;
        let cell_height = u32::from(detail.cell_pixels.1);
        if first > 0 || last < usize::from(image.cells.area.height) {
            placement.crop =
                Some(first as u32 * cell_height..last as u32 * cell_height);
        }
        Some(placement)
    }

    fn projected_image(
        &self,
        id: String,
        key: &ImageKey,
        column: Rect,
        center: bool,
    ) -> Option<Placement> {
        let image = self.images.thumbnail(key)?;
        let png = image.overlay.as_ref()?;
        if image.cells.area.is_empty() || column.is_empty() {
            return None;
        }
        let top = if center {
            column.height.saturating_sub(image.cells.area.height) / 2
        } else {
            0
        };
        Some(Placement {
            id,
            area: Rect::new(
                column.x
                    + column.width.saturating_sub(image.cells.area.width) / 2,
                column.y + top,
                image.cells.area.width.min(column.width),
                image.cells.area.height.min(column.height),
            ),
            image: png.clone(),
            crop: None,
        })
    }
}
