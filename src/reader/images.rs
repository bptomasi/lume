//! Fixed image slots keep loading independent of text offsets and scroll.

use std::ops::Range;

use pulldown_cmark::{Event, TagEnd};
use ratatui::text::Line;

use super::Builder;

/// Raster rows reserved per image; captions remain selectable text.
pub const IMAGE_ROWS: usize = 12;

/// A publisher image and its fixed display rows in one rendered article.
#[derive(Clone, Debug)]
pub struct ImageSlot {
    /// Publisher URL, resolved against the article before fetching.
    pub url: String,
    /// Plain-text alternative description.
    pub alt: String,
    /// Caption display row.
    pub caption: usize,
    /// Reserved raster display rows, independent of loading success.
    pub rows: Range<usize>,
    /// Original Markdown image range.
    pub source: Range<usize>,
    /// Trusted local formula data; publisher image markup cannot supply this.
    pub formula: Option<super::FormulaImage>,
}

pub(super) struct PendingImage {
    pub(super) url: String,
    pub(super) alt: String,
    pub(super) source: Range<usize>,
}

impl Builder<'_> {
    pub(super) fn image_event(&mut self, event: &Event<'_>) -> bool {
        let Some(image) = &mut self.image else {
            return false;
        };
        match event {
            Event::End(TagEnd::Image) => self.finish_image(),
            Event::Text(text) | Event::Code(text) => image.alt.push_str(text),
            Event::SoftBreak | Event::HardBreak => image.alt.push(' '),
            _ => {}
        }
        true
    }

    pub(super) fn finish_image(&mut self) {
        let Some(image) = self.image.take() else {
            return;
        };
        let alt = crate::content::label(&image.alt, 300);
        let title = if alt.is_empty() { "Image" } else { &alt };
        self.links.text(title);
        let caption = self.rendered.lines.len();
        self.generated(Line::raw(format!("▧ {title}")), image.source.clone());
        let start = self.rendered.lines.len();
        for _ in 0..IMAGE_ROWS {
            self.generated(Line::raw(""), image.source.clone());
        }
        self.rendered.images.push(ImageSlot {
            url: image.url,
            alt,
            caption,
            rows: start..start + IMAGE_ROWS,
            source: image.source,
            formula: None,
        });
    }

    pub(super) fn formula_image(
        &mut self,
        formula: super::FormulaImage,
        text: String,
        source: Range<usize>,
    ) {
        self.flush_list_content();
        let caption = self.rendered.lines.len();
        let start = self.rendered.text.len();
        let end = start + text.len();
        self.generated(Line::raw(text.clone()), source.clone());
        self.rendered
            .mappings
            .push(super::SourceMapping::new(start..end, source.clone()));
        for _ in 1..formula.rows {
            self.generated(Line::raw(""), source.clone());
        }
        self.rendered.images.push(ImageSlot {
            url: formula.identity.clone(),
            alt: text,
            caption,
            rows: caption..caption + formula.rows,
            source,
            formula: Some(formula),
        });
    }
}
