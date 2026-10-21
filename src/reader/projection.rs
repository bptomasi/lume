//! Preserve parser offsets through styling, entities and soft wrapping.

use std::ops::Range;

use ratatui::{
    style::Style,
    text::{Line, Span},
};

/// Correspondence between readable text and the original Markdown token.
#[derive(Clone, Debug)]
pub struct SourceMapping {
    /// Byte range before soft wrapping.
    pub display: Range<usize>,
    /// Original parser-owned UTF-8 byte range.
    pub source: Range<usize>,
}

impl SourceMapping {
    pub(super) fn new(display: Range<usize>, source: Range<usize>) -> Self {
        Self { display, source }
    }
}

/// Project selected readable bytes into their originating Markdown.
pub fn to_source(
    mappings: &[SourceMapping],
    visible: Range<usize>,
) -> Option<Range<usize>> {
    let mut result: Option<Range<usize>> = None;
    for mapping in mappings {
        let Some(range) = project(&mapping.display, &mapping.source, &visible)
        else {
            continue;
        };
        result = Some(result.map_or(range.clone(), |previous| {
            previous.start.min(range.start)..previous.end.max(range.end)
        }));
    }
    result
}

/// Project a source quotation into readable fragments, excluding markup.
pub fn to_display(
    mappings: &[SourceMapping],
    source: Range<usize>,
) -> Vec<Range<usize>> {
    mappings
        .iter()
        .filter_map(|mapping| {
            project(&mapping.source, &mapping.display, &source)
        })
        .collect()
}

/// Source interval index for projecting many annotations through one rendering.
/// Supports overlapping and reordered tokens, including expanded table cells.
pub struct SourceIndex<'a> {
    mappings: &'a [SourceMapping],
    order: Vec<usize>,
    prefix_end: Vec<usize>,
}

impl<'a> SourceIndex<'a> {
    /// Build once per rendering; annotation queries borrow its immutable tokens.
    pub fn new(mappings: &'a [SourceMapping]) -> Self {
        let mut order = (0..mappings.len()).collect::<Vec<_>>();
        order.sort_unstable_by_key(|&index| mappings[index].source.start);
        let mut maximum = 0;
        let prefix_end = order
            .iter()
            .map(|&index| {
                maximum = maximum.max(mappings[index].source.end);
                maximum
            })
            .collect();
        Self {
            mappings,
            order,
            prefix_end,
        }
    }

    /// Return intersecting readable fragments in their original display order.
    pub fn to_display(&self, source: Range<usize>) -> Vec<Range<usize>> {
        let first = self.prefix_end.partition_point(|&end| end <= source.start);
        let last = self.order.partition_point(|&index| {
            self.mappings[index].source.start < source.end
        });
        if first >= last {
            return Vec::new();
        }
        let mut hits = self.order[first..last]
            .iter()
            .filter_map(|&index| {
                let mapping = &self.mappings[index];
                project(&mapping.source, &mapping.display, &source)
                    .map(|range| (index, range))
            })
            .collect::<Vec<_>>();
        hits.sort_unstable_by_key(|(index, _)| *index);
        hits.into_iter().map(|(_, range)| range).collect()
    }
}

fn project(
    from: &Range<usize>,
    to: &Range<usize>,
    selected: &Range<usize>,
) -> Option<Range<usize>> {
    let start = from.start.max(selected.start);
    let end = from.end.min(selected.end);
    if start >= end {
        return None;
    }
    if from.len() != to.len() {
        // Entity/escape normalization is indivisible at the parser token.
        return Some(to.clone());
    }
    Some(to.start + start - from.start..to.start + end - from.start)
}

/// Patch a UTF-8 byte range while preserving syntax colors outside it.
pub fn style_range(
    line: &mut Line<'static>,
    range: Range<usize>,
    style: Style,
) {
    let mut offset = 0;
    let mut spans = Vec::new();
    for span in &line.spans {
        let length = span.content.len();
        let start = span.content.floor_char_boundary(
            range.start.saturating_sub(offset).min(length),
        );
        let end = span
            .content
            .floor_char_boundary(range.end.saturating_sub(offset).min(length));
        for (piece, selected) in [
            (0..start, false),
            (start..end.max(start), true),
            (end.max(start)..length, false),
        ] {
            if let Some(text) = span.content.get(piece)
                && !text.is_empty()
            {
                spans.push(Span::styled(
                    text.to_owned(),
                    if selected {
                        span.style.patch(style)
                    } else {
                        span.style
                    },
                ));
            }
        }
        offset += length;
    }
    line.spans = spans;
}
