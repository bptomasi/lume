//! Borrow the active reading's image slots without copying article state.

use crate::{app::App, reader::images::ImageSlot};

pub(crate) struct Viewport<'a> {
    pub(crate) slots: &'a [ImageSlot],
    pub(crate) scroll: usize,
    pub(crate) width: usize,
    pub(crate) height: usize,
}

impl Viewport<'_> {
    pub(crate) fn visible(&self) -> &[ImageSlot] {
        visible(self.slots, self.scroll, self.height)
    }
}

pub(super) fn visible(
    slots: &[ImageSlot],
    scroll: usize,
    height: usize,
) -> &[ImageSlot] {
    if height == 0 {
        return &[];
    }
    let first = slots.partition_point(|slot| slot.rows.end <= scroll);
    let remaining = &slots[first..];
    let bottom = scroll.saturating_add(height);
    let count = remaining.partition_point(|slot| slot.caption < bottom);
    &remaining[..count]
}

impl App {
    pub(crate) fn image_viewport(&self) -> Viewport<'_> {
        if self.web_active() {
            let page = self.web_page();
            return Viewport {
                slots: page.map_or(&[], |page| page.rendered.images.as_slice()),
                scroll: page.map_or(0, |page| page.scroll),
                width: page.map_or(1, |page| page.width),
                height: usize::from(self.screen.height.saturating_sub(6)),
            };
        }
        Viewport {
            slots: &self.images.slots,
            scroll: self.scroll,
            width: self.reader_width,
            height: self.reader_height,
        }
    }
}
