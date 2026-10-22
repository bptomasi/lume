//! Read caches produced by the early HTML list normalizer without rewriting them.
//!
//! CommonMark identifies the empty items and detached paragraphs. Only repeated
//! pairs with the old emitter's exact three-newline separators qualify. A
//! temporary view joins each marker to its paragraph; parser ranges project
//! back to the original document so saved quotations keep their byte authority.

use std::{borrow::Cow, ops::Range};

use pulldown_cmark::{Event, Options, Parser, Tag};

pub(super) struct Input<'a> {
    text: Cow<'a, str>,
    joins: Vec<usize>,
}

struct Item {
    marker: usize,
    paragraph_end: usize,
}

impl<'a> Input<'a> {
    pub(super) fn new(source: &'a str) -> Self {
        let mut input = Self {
            text: Cow::Borrowed(source),
            joins: Vec::new(),
        };
        if !source.contains("\n\n\n-\n\n") {
            return input;
        }
        let items = detached_items(source);
        let mut text = String::new();
        let mut copied = 0;
        for run in items
            .chunk_by(|previous, next| adjacent(source, previous, next))
            .filter(|run| run.len() >= 2)
        {
            for item in run {
                let gap = item.marker + 1;
                text.push_str(&source[copied..gap]);
                input.joins.push(text.len());
                text.push(' ');
                copied = gap + 2;
            }
        }
        if !input.joins.is_empty() {
            text.push_str(&source[copied..]);
            input.text = Cow::Owned(text);
        }
        input
    }

    pub(super) fn text(&self) -> &str {
        &self.text
    }

    pub(super) fn source_range(&self, range: Range<usize>) -> Range<usize> {
        // Each join replaces two original LF bytes with one parser-view space.
        let start = self.joins.partition_point(|join| *join < range.start);
        let end = self.joins.partition_point(|join| *join < range.end);
        range.start + start..range.end + end
    }
}

fn detached_items(source: &str) -> Vec<Item> {
    let mut marker = None;
    let mut items = Vec::new();
    for (event, range) in
        Parser::new_ext(source, Options::ENABLE_TABLES).into_offset_iter()
    {
        match event {
            Event::Start(Tag::Item) => {
                marker = legacy_marker(source, &range).then_some(range.start);
            }
            Event::Start(Tag::Paragraph) => {
                if let Some(start) = marker.take()
                    && range.start == start + 3
                {
                    items.push(Item {
                        marker: start,
                        paragraph_end: range.end,
                    });
                }
            }
            _ => {}
        }
    }
    items
}

fn legacy_marker(source: &str, range: &Range<usize>) -> bool {
    source
        .get(range.clone())
        .is_some_and(|item| item.trim() == "-")
        && source[range.start..].starts_with("-\n\n")
        && (range.start == 0 || source[..range.start].ends_with("\n\n\n"))
}

fn adjacent(source: &str, previous: &Item, next: &Item) -> bool {
    source
        .get(previous.paragraph_end..next.marker)
        .is_some_and(|gap| {
            gap.len() <= 3 && gap.bytes().all(|byte| byte == b'\n')
        })
}
