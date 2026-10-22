//! CommonMark-owned structure for reader navigation and exact code copies.

use std::ops::Range;

mod navigation;

pub use navigation::{Link, Navigation};

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

/// One navigable heading, retaining its original source anchor.
#[derive(Clone, Debug)]
pub struct Heading {
    /// Plain heading text.
    pub title: String,
    /// Heading level, from one through six.
    pub level: u8,
    /// Original Markdown byte range.
    pub range: Range<usize>,
}

/// A code block's decoded contents, without Markdown fences.
#[derive(Clone, Debug)]
pub struct CodeBlock {
    /// Original Markdown range, used to locate the block under the cursor.
    pub range: Range<usize>,
    /// Unwrapped contents emitted by the parser.
    pub text: String,
}

/// Width-independent reader structure; never parsed while drawing.
#[derive(Clone, Debug, Default)]
pub struct DocumentIndex {
    /// Link occurrences and section destinations independent of display width.
    pub navigation: Navigation,
    /// Headings in encounter order.
    pub headings: Vec<Heading>,
    /// Code blocks in encounter order.
    pub code: Vec<CodeBlock>,
    /// Raw table ranges; dedicated viewing preserves their full width.
    pub tables: Vec<Range<usize>>,
    /// Structural indexing stopped after its bounded event budget.
    pub limited: bool,
}

impl DocumentIndex {
    /// Index bounded, already sanitized Markdown using the qualified parser.
    pub fn parse(body: &str) -> Self {
        Self::parse_with_options(body, super::RenderOptions::default())
    }

    /// Index using the same formula policy as the visible reader.
    #[expect(
        clippy::too_many_lines,
        reason = "Paired structural parser states share one bounded scan"
    )]
    pub fn parse_with_options(
        body: &str,
        options: super::RenderOptions,
    ) -> Self {
        let mut index = Self::default();
        let mut heading = None;
        let mut code = None;
        for (count, (event, range)) in
            Parser::new_ext(body, Options::ENABLE_TABLES | Options::ENABLE_MATH)
                .into_offset_iter()
                .enumerate()
        {
            if count == 50_000 {
                index.limited = true;
                break;
            }
            index.navigation.event(&event, range.clone());
            match event {
                Event::Start(Tag::Heading { level, .. }) => {
                    heading = Some(Heading {
                        title: String::new(),
                        level: level as u8,
                        range,
                    });
                }
                Event::End(TagEnd::Heading(_)) => {
                    if let Some(item) = heading.take() {
                        index.navigation.heading(&item.title, item.range.start);
                        index.headings.push(item);
                    }
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    code = Some(CodeBlock {
                        range,
                        text: String::new(),
                    });
                }
                Event::End(TagEnd::CodeBlock) => {
                    if let Some(item) = code.take() {
                        index.code.push(item);
                    }
                }
                Event::Start(Tag::Table(_)) => index.tables.push(range),
                Event::Text(text) | Event::Code(text) => {
                    append_text(&mut heading, &mut code, &text);
                }
                Event::InlineMath(text) | Event::DisplayMath(text)
                    if heading.is_some() && options.latex_formulas =>
                {
                    let text = super::math::Math::default()
                        .render(&text, false)
                        .unwrap_or_else(|| text.into_string());
                    append_text(&mut heading, &mut code, &text);
                }
                Event::InlineMath(_) | Event::DisplayMath(_) => {
                    append_text(
                        &mut heading,
                        &mut code,
                        body.get(range).unwrap_or_default(),
                    );
                }
                Event::SoftBreak | Event::HardBreak => {
                    append_text(&mut heading, &mut code, " ");
                }
                _ => {}
            }
        }
        index
    }
}

fn append_text(
    heading: &mut Option<Heading>,
    code: &mut Option<CodeBlock>,
    text: &str,
) {
    if let Some(heading) = heading {
        heading.title.push_str(text);
    }
    if let Some(code) = code {
        code.text.push_str(text);
    }
}
