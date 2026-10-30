//! Image clicks open detail without starting a text-selection capture.

use crossterm::event::MouseEvent;
use ratatui::layout::{Position, Rect};

use crate::ui;

use super::super::{App, ImageInspector, Mode};

impl App {
    pub(super) fn open_pointer_image(
        &mut self,
        reader: Rect,
        event: MouseEvent,
    ) -> bool {
        let column = ui::workspace_reader_content(reader, &self.workspace);
        self.open_pointer_image_column(column, event)
    }

    pub(in crate::app) fn open_pointer_image_column(
        &mut self,
        column: Rect,
        event: MouseEvent,
    ) -> bool {
        if self.web_active() && self.web.opening {
            return false;
        }
        if !column.contains(Position::new(event.column, event.row)) {
            return false;
        }
        let viewport = self.image_viewport();
        let row = viewport.scroll + usize::from(event.row - column.y);
        let image = viewport.visible().iter().find_map(|slot| {
            let key = self.image_key(slot)?;
            let caption = row == slot.caption;
            let pixels = self.images.thumbnail(&key).is_some_and(|image| {
                let left = column.x
                    + column.width.saturating_sub(image.cells.area.width) / 2;
                row >= slot.rows.start
                    && row
                        < slot.rows.start + usize::from(image.cells.area.height)
                    && event.column >= left
                    && event.column
                        < left + image.cells.area.width.min(column.width)
            });
            (caption || pixels).then(|| ImageInspector {
                url: key.url,
                title: slot.alt.clone(),
                formula: slot.formula.clone(),
                zoom: 1,
                pan: [0, 0],
            })
        });
        if let Some(image) = image {
            self.vim.clear_input();
            self.mode = Mode::Image(Box::new(image));
            return true;
        }
        false
    }
}
