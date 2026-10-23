//! Exact source/options/width keys retain at most three prior layouts and 8 MiB.
use super::Completed;
use std::{collections::VecDeque, mem::size_of};

#[derive(Default)]
pub(super) struct Cache {
    entries: VecDeque<(Completed, usize)>,
    bytes: usize,
}

impl Cache {
    pub(super) fn take(
        &mut self,
        body: &str,
        width: usize,
        options: crate::reader::RenderOptions,
        revision: u64,
    ) -> Option<Completed> {
        let index = self.entries.iter().position(|(entry, _)| {
            entry.revision == revision
                && entry.width == width
                && entry.options == options
                && entry.body.as_ref() == body
        })?;
        let (entry, bytes) = self.entries.remove(index)?;
        self.bytes -= bytes;
        Some(entry)
    }

    pub(super) fn insert(&mut self, entry: Completed) {
        let bytes = retained_bytes(&entry);
        if bytes > 8 * 1024 * 1024 {
            return;
        }
        while self.entries.len() >= 3 || self.bytes + bytes > 8 * 1024 * 1024 {
            let Some((_, old)) = self.entries.pop_back() else {
                break;
            };
            self.bytes -= old;
        }
        self.entries.push_front((entry, bytes));
        self.bytes += bytes;
    }

    pub(super) fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }
}

fn retained_bytes(entry: &Completed) -> usize {
    let rendered = &entry.rendered;
    let lines = rendered
        .lines
        .iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|span| {
                    size_of::<ratatui::text::Span<'_>>() + span.content.len()
                })
                .sum::<usize>()
        })
        .sum::<usize>();
    let images = rendered.images.iter().map(image_bytes).sum::<usize>();
    entry.body.len()
        + rendered.text.len()
        + lines
        + images
        + rendered.ranges.len() * size_of::<std::ops::Range<usize>>() * 2
        + rendered.lines.len() * (size_of::<ratatui::text::Line<'_>>() + 24)
        + rendered.mappings.len() * size_of::<crate::reader::SourceMapping>()
        + rendered
            .links
            .iter()
            .map(|(url, title)| url.len() + title.len())
            .sum::<usize>()
        + entry
            .blocks
            .iter()
            .map(|block| block.text.len() + block.language.len())
            .sum::<usize>()
}

fn image_bytes(slot: &crate::reader::images::ImageSlot) -> usize {
    size_of::<crate::reader::images::ImageSlot>()
        + slot.url.len()
        + slot.alt.len()
        + slot
            .formula
            .as_ref()
            .map_or(0, |formula| formula.svg.len() + formula.latex.len())
}
