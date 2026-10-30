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
mod legacy_lists;
mod links;
mod lists;
mod live;
mod math;
pub use math::graphics::FormulaImage;
pub mod motion;
mod projection;
mod syntax;
pub mod table_view;
mod tables;
pub(crate) mod viewport;
mod wrapping;

pub use live::LiveRenderer;
pub(crate) use projection::display_start;
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

/// Source presentation policy shared by synchronous and background readers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RenderOptions {
    /// Convert supported LaTeX to selectable Unicode; otherwise keep raw source.
    pub latex_formulas: bool,
    /// Prepare optional SVG formula blocks in the background layout worker.
    pub graphical_formulas: bool,
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            latex_formulas: true,
            graphical_formulas: false,
        }
    }
}

impl RenderOptions {
    /// Text-only surfaces retain semantic math without allocating pixel slots.
    pub fn text_only(self) -> Self {
        Self {
            graphical_formulas: false,
            ..self
        }
    }
}

/// Immutable, width-specific presentation consumed without parsing during draw.
#[derive(Clone, Default)]
pub struct Rendered {
    /// Styled rows before applying the corresponding visual `insets`.
    pub lines: Vec<Line<'static>>,
    /// Original Markdown byte range for every row.
    pub ranges: Vec<Range<usize>>,
    /// Rows that preserve source columns and allow horizontal scrolling.
    pub code: Vec<bool>,
    /// Original source-byte block identity for independent horizontal scrolling.
    pub horizontal_groups: Vec<Option<usize>>,
    /// Nonfinal prose rows eligible for visual justification.
    pub justifiable: Vec<bool>,
    /// Visual continuation padding, excluded from readable/copied bytes.
    pub insets: Vec<usize>,
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
    cancel: Option<&'a crate::cancellation::Cancellation>,
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
    lists: lists::Lists,
    horizontal_group: Option<usize>,
    links: links::Links,
    image: Option<images::PendingImage>,
    table: Option<tables::Pending>,
    table_bytes: usize,
    math: math::Math,
    options: RenderOptions,
    highlight: &'a mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
}

/// Parse and lay out bounded stored Markdown for a terminal width.
pub fn render(body: &str, width: usize) -> Rendered {
    render_with_options(body, width, RenderOptions::default())
}

/// Lay out source using an explicit formula policy, preserving original anchors.
pub fn render_with_options(
    body: &str,
    width: usize,
    options: RenderOptions,
) -> Rendered {
    render_with(body, width, options, &mut syntax::highlight)
}

pub(super) fn render_with(
    body: &str,
    width: usize,
    options: RenderOptions,
    highlight: &mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
) -> Rendered {
    render_body(body, width, options, highlight, None)
}

pub(crate) fn render_cancellable(
    body: &str,
    width: usize,
    options: RenderOptions,
    highlight: &mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
    cancel: &crate::cancellation::Cancellation,
) -> anyhow::Result<Rendered> {
    cancel.check()?;
    let rendered = render_body(body, width, options, highlight, Some(cancel));
    cancel.check()?;
    Ok(rendered)
}

fn render_body<'a>(
    body: &'a str,
    width: usize,
    options: RenderOptions,
    highlight: &'a mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
    cancel: Option<&'a crate::cancellation::Cancellation>,
) -> Rendered {
    let input = legacy_lists::Input::new(body);
    let mut builder = Builder::new(body, width, options, highlight, cancel);
    for (event, range) in Parser::new_ext(
        input.text(),
        Options::ENABLE_TABLES | Options::ENABLE_MATH,
    )
    .into_offset_iter()
    {
        if builder.cancelled() {
            return Rendered::default();
        }
        builder.event(event, input.source_range(range));
    }
    builder.flush();
    builder.rendered.links = builder.links.finish();
    builder.rendered
}

impl<'a> Builder<'a> {
    fn new(
        body: &'a str,
        width: usize,
        options: RenderOptions,
        highlight: &'a mut dyn FnMut(&str, &str) -> Vec<Line<'static>>,
        cancel: Option<&'a crate::cancellation::Cancellation>,
    ) -> Self {
        Self {
            source: body,
            cancel,
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
            lists: lists::Lists::default(),
            horizontal_group: None,
            links: links::Links::default(),
            image: None,
            table: None,
            table_bytes: 0,
            math: math::Math::default(),
            options,
            highlight,
        }
    }

    fn cancelled(&self) -> bool {
        self.cancel.is_some_and(|cancel| cancel.is_cancelled())
    }
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
                self.text_prefix();
                let source = self.inline_code_source(&text, range);
                self.map_text(&text, source);
                self.spans.push(Span::styled(
                    text.into_string(),
                    Style::new().fg(AMBER).bg(Color::Rgb(34, 32, 30)),
                ));
            }
            Event::InlineMath(text) => self.append_math(&text, range, false),
            Event::DisplayMath(text) => self.append_math(&text, range, true),
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

    fn append_math(&mut self, latex: &str, range: Range<usize>, block: bool) {
        if block {
            self.flush();
        }
        let projected = self
            .options
            .latex_formulas
            .then(|| self.math.render(latex, block))
            .flatten();
        let spatial = projected.as_ref().is_some_and(|text| text.spatial);
        let modifier = projected
            .as_ref()
            .map_or(Modifier::empty(), |text| text.modifier);
        let text = projected.map_or_else(
            || self.source.get(range.clone()).unwrap_or(latex).to_owned(),
            |text| text.value,
        );
        if self.options.latex_formulas
            && self.options.graphical_formulas
            && self.table.is_none()
            && (block || spatial)
            && self
                .rendered
                .images
                .iter()
                .filter(|image| image.formula.is_some())
                .count()
                < 32
            && let Some(formula) = self.math.graphic(latex, block)
        {
            self.formula_image(formula, text, range);
            return;
        }
        self.push_style(modifier);
        self.append(text, range);
        self.styles.pop();
        if block {
            self.flush();
        }
    }

    fn horizontal_rule(&mut self, range: Range<usize>) {
        self.flush_list_content();
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
            if let Some(inset) = self.rendered.insets.last_mut() {
                *inset = self.lists.indent().min(self.width.saturating_sub(1));
            }
            offset += source.len();
        }
    }

    fn start(&mut self, tag: Tag<'_>, range: Range<usize>) {
        match tag {
            Tag::Paragraph | Tag::Heading { .. } | Tag::CodeBlock(_) => {
                self.start_block(tag, range);
            }
            Tag::List(start) => {
                // An empty parent only supplies indentation for its children.
                self.flush();
                self.lists.discard_marker();
                self.lists.begin(start);
            }
            Tag::Item => {
                self.flush();
                self.start = range.start;
                self.end = range.end;
                self.lists.item();
            }
            Tag::BlockQuote(_) => self.quote += 1,
            Tag::Emphasis => self.push_style(Modifier::ITALIC),
            Tag::Strong => self.push_style(Modifier::BOLD),
            Tag::Link { dest_url, .. } => self.begin_link(&dest_url),
            Tag::Image { dest_url, .. }
                if self
                    .rendered
                    .images
                    .iter()
                    .filter(|image| image.formula.is_none())
                    .count()
                    < 32 =>
            {
                self.flush_list_content();
                self.image = Some(images::PendingImage {
                    url: dest_url.into_string(),
                    alt: String::new(),
                    source: range,
                });
            }
            _ => {}
        }
    }

    fn start_block(&mut self, tag: Tag<'_>, range: Range<usize>) {
        if matches!(&tag, Tag::CodeBlock(_)) {
            self.flush_list_content();
        } else {
            self.flush();
        }
        self.paragraph = matches!(&tag, Tag::Paragraph);
        self.start = range.start;
        self.end = range.end;
        if matches!(&tag, Tag::CodeBlock(_)) {
            self.horizontal_group = Some(range.start);
        }
        self.block_style(tag);
    }

    fn begin_link(&mut self, address: &str) {
        self.text_prefix();
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
                self.flush();
                self.lists.end();
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
            TagEnd::Item => {
                // Publisher spacer items and anchor-only items have no visible
                // content. Keep source bytes authoritative without painting a
                // dangling marker. Blocks emit their marker when they begin.
                self.flush();
                self.lists.discard_marker();
            }
            _ => {}
        }
    }

    fn push_style(&mut self, modifier: Modifier) {
        let style = self.styles.last().copied().unwrap_or_default();
        self.styles.push(style.add_modifier(modifier));
    }

    fn append(&mut self, text: String, range: Range<usize>) {
        if self.spans.is_empty()
            && self.lists.pending()
            && text.trim().is_empty()
        {
            return;
        }
        self.text_prefix();
        if self.spans.is_empty() {
            self.start = range.start;
        }
        self.end = self.end.max(range.end);
        self.map_text(&text, range);
        let style = self.styles.last().copied().unwrap_or_default();
        self.spans.push(Span::styled(text, style));
    }

    fn text_prefix(&mut self) {
        if !self.spans.is_empty() || self.table.is_some() {
            return;
        }
        if self.quote > 0 {
            self.spans.push(Span::styled("│ ", Style::new().fg(AMBER)));
            self.text_length += "│ ".len();
        }
        let prefix = self.lists.prefix();
        if !prefix.is_empty() {
            self.text_length += prefix.len();
            self.spans.push(Span::raw(prefix));
        }
    }

    fn flush_list_content(&mut self) {
        if self.lists.pending() {
            self.text_prefix();
        }
        self.flush();
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
        let inset = self.lists.indent().min(self.width.saturating_sub(1));
        let padding = " ".repeat(inset);
        let options =
            textwrap::Options::new(self.width).subsequent_indent(&padding);
        let rows = textwrap::wrap(text, options);
        let count = rows.len();
        for (index, wrapped) in rows.into_iter().enumerate() {
            if self.cancelled() {
                break;
            }
            let wrapped = if index == 0 {
                wrapped.as_ref()
            } else {
                wrapped.strip_prefix(&padding).unwrap_or(&wrapped)
            };
            let start = consumed + text[consumed..].find(wrapped).unwrap_or(0);
            let end = start + wrapped.len();
            let (line, source) = paragraph.row(start..end);
            self.push(
                line,
                source.unwrap_or(self.start..self.end),
                code,
                base + start..base + end,
            );
            if index > 0
                && let Some(value) = self.rendered.insets.last_mut()
            {
                *value = inset;
            }
            if let Some(eligible) = self.rendered.justifiable.last_mut() {
                *eligible = self.paragraph
                    && !code
                    && self.quote == 0
                    && self.lists.is_empty()
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
        self.rendered.insets.push(0);
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
    render_with(body, 80, RenderOptions::default(), &mut |text, _| {
        text.lines()
            .map(|line| Line::raw(line.to_owned()))
            .collect()
    })
    .links
}

#[cfg(test)]
mod cancellation_tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn superseded_layout_stops_before_preparing_the_remaining_code_blocks() {
        let (generation, cancel) = crate::cancellation::test_token();
        let body = "```rust\nlet value = 1;\n```\n\n".repeat(100);
        let mut calls = 0;
        let result = render_cancellable(
            &body,
            80,
            RenderOptions::default(),
            &mut |_, _| {
                calls += 1;
                generation.store(1, Ordering::Release);
                Vec::new()
            },
            &cancel,
        );
        assert!(result.is_err());
        assert_eq!(calls, 1);
        assert!(render(&body, 80).text.contains("let value = 1;"));
    }
}
