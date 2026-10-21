//! Cached Markdown layout and source mapping.
//!
//! CommonMark owns parsing. Prose wraps at terminal cell widths; fenced code
//! keeps its columns. Interactive consumers defer colors through LiveRenderer;
//! synchronous consumers use render. Source byte ranges
//! let annotations capture original text rather than a lossy rendered copy.

pub mod cursor;
pub mod images;
pub mod index;
mod justification;
mod language;
mod layout_worker;
mod links;
mod live;
pub mod motion;
mod projection;
mod syntax;
pub mod table_view;
mod tables;
pub(crate) mod viewport;
mod wrapping;

pub use live::LiveRenderer;
pub use projection::{
    SourceIndex, SourceMapping, style_range, to_display, to_source,
};

use std::ops::Range;

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use ratatui::{
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
};

const TEXT: Color = Color::Rgb(224, 219, 208);
const AMBER: Color = Color::Rgb(225, 179, 109);
const BLUE: Color = Color::Rgb(145, 180, 218);

/// Immutable, width-specific presentation consumed without parsing during draw.
#[derive(Clone, Default)]
pub struct Rendered {
    /// Styled display rows.
    pub lines: Vec<Line<'static>>,
    /// Original Markdown byte range for every row.
    pub ranges: Vec<Range<usize>>,
    /// Rows that preserve source columns and allow horizontal scrolling.
    pub code: Vec<bool>,
    /// Original source-byte block identity for independent horizontal scrolling.
    pub horizontal_groups: Vec<Option<usize>>,
    /// Nonfinal prose rows eligible for visual justification.
    pub justifiable: Vec<bool>,
    /// Display row ranges of terminal tables, sharing horizontal code navigation.
    pub table_rows: Vec<Range<usize>>,
    /// Readable text before soft wrapping; copied selections use these bytes.
    pub text: String,
    /// Byte ranges in readable text for each display row.
    pub text_ranges: Vec<Range<usize>>,
    /// Parser-owned source correspondence before wrapping.
    pub mappings: Vec<SourceMapping>,
    /// Unique, validated HTTP(S) links in encounter order.
    pub links: Vec<(String, String)>,
    /// Fixed publisher image slots, loaded separately from text layout.
    pub images: Vec<images::ImageSlot>,
}

struct Builder<'a> {
    source: &'a str,
    rendered: Rendered,
    spans: Vec<Span<'static>>,
    mappings: Vec<SourceMapping>,
    text_length: usize,
    start: usize,
    end: usize,
    width: usize,
    styles: Vec<Style>,
    code: Option<String>,
    paragraph: bool,
    quote: usize,
    list_depth: usize,
    horizontal_group: Option<usize>,
    links: links::Links,
    image: Option<images::PendingImage>,
    table: Option<tables::Pending>,
    table_bytes: usize,
    highlight: &'a mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
}

/// Parse and lay out bounded stored Markdown for a terminal width.
pub fn render(body: &str, width: usize) -> Rendered {
    render_with(body, width, &mut syntax::highlight)
}

pub(super) fn render_with(
    body: &str,
    width: usize,
    highlight: &mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
) -> Rendered {
    let mut builder = Builder {
        source: body,
        rendered: Rendered::default(),
        spans: Vec::new(),
        mappings: Vec::new(),
        text_length: 0,
        start: 0,
        end: 0,
        width: width.max(1),
        styles: vec![Style::new().fg(TEXT)],
        code: None,
        paragraph: false,
        quote: 0,
        list_depth: 0,
        horizontal_group: None,
        links: links::Links::default(),
        image: None,
        table: None,
        table_bytes: 0,
        highlight,
    };
    for (event, range) in
        Parser::new_ext(body, Options::ENABLE_TABLES).into_offset_iter()
    {
        builder.event(event, range);
    }
    builder.flush();
    builder.rendered.links = builder.links.finish();
    builder.rendered
}

impl Builder<'_> {
    fn event(&mut self, event: Event<'_>, range: Range<usize>) {
        if self.table_event(&event, &range) || self.image_event(&event) {
            return;
        }
        match event {
            Event::Start(tag) => self.start(tag, range),
            Event::End(tag) => self.end(tag),
            Event::Text(text) if self.code.is_some() => {
                self.append_code(&text, range);
            }
            Event::Html(ref text) | Event::InlineHtml(ref text)
                if crate::content::anchors::invisible(text) => {}
            Event::Text(text) | Event::Html(text) | Event::InlineHtml(text) => {
                self.append(text.into_string(), range);
            }
            Event::Code(text) => {
                self.quote_prefix();
                let source = self.inline_code_source(&text, range);
                self.map_text(&text, source);
                self.spans.push(Span::styled(
                    text.into_string(),
                    Style::new().fg(AMBER).bg(Color::Rgb(34, 32, 30)),
                ));
            }
            Event::SoftBreak => self.append(" ".into(), range),
            Event::HardBreak => {
                self.links.text(" ");
                self.flush();
            }
            Event::Rule => self.horizontal_rule(range),
            Event::TaskListMarker(done) => {
                self.append(if done { "☑ " } else { "☐ " }.into(), range);
            }
            _ => {}
        }
    }

    fn horizontal_rule(&mut self, range: Range<usize>) {
        self.flush();
        self.generated(
            Line::styled(
                "─".repeat(self.width.min(40)),
                Style::new().fg(AMBER),
            ),
            range,
        );
    }

    fn append_code(&mut self, text: &str, range: Range<usize>) {
        self.flush();
        let base = self.rendered.text.len();
        self.rendered.text.push_str(text);
        if !text.ends_with('\n') {
            self.rendered.text.push('\n');
        }
        self.rendered
            .mappings
            .push(SourceMapping::new(base..base + text.len(), range.clone()));
        let language = self.code.as_deref().unwrap_or_default();
        let language = language::resolve(text, language);
        let mut offset = 0;
        for (line, source) in (self.highlight)(text, &language)
            .into_iter()
            .zip(text.split_inclusive('\n'))
        {
            let end = offset + source.trim_end_matches('\n').len();
            let source_range = if range.len() == text.len() {
                range.start + offset..range.start + end
            } else {
                range.clone()
            };
            self.push(line, source_range, true, base + offset..base + end);
            offset += source.len();
        }
    }

    fn start(&mut self, tag: Tag<'_>, range: Range<usize>) {
        match tag {
            Tag::Paragraph | Tag::Heading { .. } | Tag::CodeBlock(_) => {
                self.flush();
                self.paragraph = matches!(&tag, Tag::Paragraph);
                self.start = range.start;
                self.end = range.end;
                if matches!(&tag, Tag::CodeBlock(_)) {
                    self.horizontal_group = Some(range.start);
                }
                self.block_style(tag);
            }
            Tag::List(_) => self.list_depth += 1,
            Tag::Item => {
                self.flush();
                self.start = range.start;
                self.quote_prefix();
                self.text_length += "• ".len();
                self.spans.push(Span::raw("• "));
            }
            Tag::BlockQuote(_) => self.quote += 1,
            Tag::Emphasis => self.push_style(Modifier::ITALIC),
            Tag::Strong => self.push_style(Modifier::BOLD),
            Tag::Link { dest_url, .. } => self.begin_link(&dest_url),
            Tag::Image { dest_url, .. } if self.rendered.images.len() < 32 => {
                self.flush();
                self.image = Some(images::PendingImage {
                    url: dest_url.into_string(),
                    alt: String::new(),
                    source: range,
                });
            }
            _ => {}
        }
    }

    fn begin_link(&mut self, address: &str) {
        self.quote_prefix();
        self.links.begin(address);
        self.styles.push(Style::new().fg(BLUE).underlined());
    }

    fn block_style(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Heading { .. } => {
                self.styles.push(Style::new().fg(AMBER).bold());
            }
            Tag::CodeBlock(kind) => {
                let language = match kind {
                    CodeBlockKind::Fenced(info) => info
                        .split_whitespace()
                        .next()
                        .unwrap_or_default()
                        .to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some(language);
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::CodeBlock => {
                self.flush();
                self.paragraph = false;
                self.generated(Line::raw(""), self.end..self.end);
                if matches!(tag, TagEnd::Heading(_)) {
                    self.styles.pop();
                }
                if tag == TagEnd::CodeBlock {
                    self.code = None;
                    self.horizontal_group = None;
                }
            }
            TagEnd::List(_) => {
                self.list_depth = self.list_depth.saturating_sub(1)
            }
            TagEnd::Link => {
                self.links.end();
                self.styles.pop();
            }
            TagEnd::Strong | TagEnd::Emphasis => {
                if self.styles.len() > 1 {
                    self.styles.pop();
                }
            }
            TagEnd::BlockQuote(_) => self.quote = self.quote.saturating_sub(1),
            TagEnd::Item => self.flush(),
            _ => {}
        }
    }

    fn push_style(&mut self, modifier: Modifier) {
        let style = self.styles.last().copied().unwrap_or_default();
        self.styles.push(style.add_modifier(modifier));
    }

    fn append(&mut self, text: String, range: Range<usize>) {
        self.quote_prefix();
        if self.spans.is_empty() {
            self.start = range.start;
        }
        self.end = self.end.max(range.end);
        self.map_text(&text, range);
        let style = self.styles.last().copied().unwrap_or_default();
        self.spans.push(Span::styled(text, style));
    }

    fn quote_prefix(&mut self) {
        if self.spans.is_empty() && self.quote > 0 && self.table.is_none() {
            self.spans.push(Span::styled("│ ", Style::new().fg(AMBER)));
            self.text_length += "│ ".len();
        }
    }

    fn inline_code_source(
        &self,
        text: &str,
        source: Range<usize>,
    ) -> Range<usize> {
        // CommonMark's inline-code range includes delimiters and optional
        // padding. Only narrow it when the visible text occurs verbatim.
        self.source
            .get(source.clone())
            .and_then(|raw| raw.find(text))
            .map_or(source.clone(), |offset| {
                source.start + offset..source.start + offset + text.len()
            })
    }

    fn map_text(&mut self, text: &str, source: Range<usize>) {
        self.links.text(text);
        let start = self.text_length;
        self.text_length += text.len();
        self.mappings
            .push(SourceMapping::new(start..self.text_length, source));
    }

    fn flush(&mut self) {
        if self.spans.is_empty() {
            return;
        }
        let mut spans = std::mem::take(&mut self.spans);
        let code = self.paragraph && code_paragraph(&mut spans);
        let mappings = std::mem::take(&mut self.mappings);
        self.text_length = 0;
        let text = spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();
        let base = self.rendered.text.len();
        self.rendered.text.push_str(&text);
        self.rendered.text.push('\n');
        self.wrap_paragraph(&spans, &mappings, &text, code, base);
        self.rendered
            .mappings
            .extend(mappings.into_iter().map(|mut map| {
                map.display.start += base;
                map.display.end += base;
                map
            }));
    }

    fn wrap_paragraph(
        &mut self,
        spans: &[Span<'static>],
        mappings: &[SourceMapping],
        text: &str,
        code: bool,
        base: usize,
    ) {
        let mut consumed = 0;
        let mut paragraph = wrapping::Paragraph::new(spans, mappings);
        let rows = textwrap::wrap(text, self.width);
        let count = rows.len();
        for (index, wrapped) in rows.into_iter().enumerate() {
            let offset = text[consumed..].find(wrapped.as_ref()).unwrap_or(0);
            let start = consumed + offset;
            let end = start + wrapped.len();
            let (line, source) = paragraph.row(start..end);
            self.push(
                line,
                source.unwrap_or(self.start..self.end),
                code,
                base + start..base + end,
            );
            if let Some(eligible) = self.rendered.justifiable.last_mut() {
                *eligible = self.paragraph
                    && !code
                    && self.quote == 0
                    && self.list_depth == 0
                    && index + 1 < count;
            }
            consumed = end;
        }
    }

    fn generated(&mut self, line: Line<'static>, source: Range<usize>) {
        let start = self.rendered.text.len();
        self.rendered.text.push_str(&line.to_string());
        let end = self.rendered.text.len();
        self.rendered.text.push('\n');
        self.push(line, source, false, start..end);
    }

    fn push(
        &mut self,
        line: Line<'static>,
        range: Range<usize>,
        code: bool,
        text: Range<usize>,
    ) {
        self.rendered.lines.push(line);
        self.rendered.ranges.push(range);
        self.rendered.code.push(code);
        self.rendered
            .horizontal_groups
            .push(code.then_some(self.horizontal_group.unwrap_or(self.start)));
        self.rendered.justifiable.push(false);
        self.rendered.text_ranges.push(text);
    }
}

fn code_paragraph(spans: &mut [Span<'static>]) -> bool {
    let background = Some(Color::Rgb(34, 32, 30));
    let code = spans.iter().any(|span| span.style.bg == background)
        && spans.iter().all(|span| {
            span.style.bg == background || span.content.trim().is_empty()
        });
    if code {
        for span in spans {
            span.style.bg = None;
        }
    }
    code
}

/// Read reference-feed links without executing syntax grammars.
pub(crate) fn render_with_plain_links(body: &str) -> Vec<(String, String)> {
    render_with(body, 80, &mut |text, _| {
        text.lines()
            .map(|line| Line::raw(line.to_owned()))
            .collect()
    })
    .links
}
