//! Borrow the active reading's image slots without copying article state.

use crate::{app::App, reader::images::ImageSlot};

#[derive(Clone, Copy)]
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

pub(super) use crate::reader::images::visible_slots as visible;

impl App {
    pub(crate) fn image_viewport(&self) -> Viewport<'_> {
        if self.web_active() {
            let page = self.web_page();
            return Viewport {
                slots: page.map_or(&[], |page| page.rendered.images.as_slice()),
                scroll: page.map_or(0, |page| page.scroll),
                width: page.map_or(1, |page| page.width),
                height: usize::from(
                    self.layout.screen().height.saturating_sub(6),
                ),
            };
        }
        Viewport {
            slots: self.reader().image_slots,
            scroll: self.reader().scroll,
            width: self.reader().reader_width,
            height: self.reader().reader_height,
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn visible_image_bounds_include_captions_clips_gaps_and_empty_viewports() {
        use super::visible;
        use crate::reader::images::ImageSlot;

        let slots = (0..10_000)
            .map(|index| {
                let caption = index * 20;
                ImageSlot {
                    formula: None,
                    url: format!("https://example.org/{index}.svg"),
                    alt: String::new(),
                    caption,
                    rows: caption + 1..caption + 13,
                    source: index..index + 1,
                }
            })
            .collect::<Vec<_>>();
        for (scroll, height, expected) in [
            (2000, 1, vec![100]),
            (2013, 7, vec![]),
            (1999, 15, vec![100]),
            (1990, 30, vec![99, 100]),
            (2000, 0, vec![]),
            (199_993, 40, vec![]),
            (usize::MAX - 1, 8, vec![]),
        ] {
            let actual = visible(&slots, scroll, height)
                .iter()
                .map(|slot| slot.source.start)
                .collect::<Vec<_>>();
            assert_eq!(actual, expected, "viewport {scroll}+{height}");
        }
    }
}
