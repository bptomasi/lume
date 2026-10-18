//! HTML5 fragment normalization into bounded, terminal-safe Markdown.
//!
//! The HTML tokenizer owns recovery and entity decoding. This adapter maps
//! readable tags to Markdown, suppresses active content and validates links.

mod availability;
mod code;
mod table;

pub use availability::{BodyKind, MISSING_BODY, body_kind};

/// Estimate reading duration at 220 words/minute; references have no duration.
pub fn reading_minutes(body: &str) -> usize {
    if body_kind(body) != BodyKind::Text {
        return 0;
    }
    body.split_whitespace().count().div_ceil(220)
}

use code::CodeBlock;
use html5gum::{DefaultEmitter, Token, Tokenizer};

/// Retain newlines and tabs while removing terminal control characters.
pub fn clean(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
        .collect()
}

/// Produce bounded, single-line metadata safe for terminal presentation.
pub fn label(text: &str, limit: usize) -> String {
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(limit)
        .collect()
}

/// Normalize HTML without a base URL, preserving plain-text feeds verbatim.
pub fn article_text(fragment: &str, is_html: bool) -> String {
    article_markdown(fragment, is_html, "")
}

/// Normalize a fragment, resolving relative links against its article URL.
pub fn article_markdown(fragment: &str, html: bool, address: &str) -> String {
    let end = fragment.floor_char_boundary(fragment.len().min(262_144));
    let mut text = if html {
        let mut fragment_state = Fragment::new(address);
        let mut emitter = DefaultEmitter::default();
        emitter.naively_switch_states(true);
        for token in
            Tokenizer::new_with_emitter(&fragment[..end], emitter).flatten()
        {
            fragment_state.event(token);
        }
        fragment_state.finish()
    } else {
        clean(&fragment[..end])
    };
    if end < fragment.len() {
        text.push_str("\n\n[Content limited to 256 KiB; open the original.]");
    }
    text
}

struct Fragment {
    text: String,
    hidden: Option<String>,
    preformatted: Option<CodeBlock>,
    quotes: usize,
    link: Option<String>,
    table: Option<table::Table>,
    base: Option<url::Url>,
}

impl Fragment {
    fn new(address: &str) -> Self {
        Self {
            text: String::new(),
            hidden: None,
            preformatted: None,
            quotes: 0,
            link: None,
            table: None,
            base: crate::source::feed_url(address).ok(),
        }
    }

    fn event(&mut self, token: Token) {
        match token {
            Token::StartTag(tag) => self.start_tag(tag),
            Token::EndTag(tag) => {
                let name = String::from_utf8_lossy(&tag.name);
                if self.hidden.as_deref() == Some(name.as_ref()) {
                    self.hidden = None;
                } else if self.hidden.is_none() {
                    if self.table.is_some() {
                        self.table_tag(&name, true);
                    } else {
                        self.tag(&name, true);
                    }
                }
            }
            Token::String(bytes) if self.hidden.is_none() => {
                self.append(&String::from_utf8_lossy(&bytes));
            }
            _ => {}
        }
    }

    fn start_tag(&mut self, tag: html5gum::StartTag<()>) {
        let name = String::from_utf8_lossy(&tag.name).into_owned();
        if self.hidden.is_some() {
            return;
        }
        if matches!(name.as_str(), "script" | "style" | "iframe" | "svg") {
            self.hidden = Some(name);
            return;
        }
        if name == "a" && self.preformatted.is_none() {
            self.begin_link(&tag);
        }
        if self.preformatted.is_none()
            && (self.table.is_some() || name == "table")
        {
            self.table_tag(&name, false);
            return;
        }
        if name == "pre" {
            self.begin_code(&tag);
            return;
        }
        if let Some(block) = &mut self.preformatted {
            if name == "code" {
                block.set_language(&tag);
            }
            return;
        }
        self.tag(&name, false);
    }

    fn begin_link(&mut self, tag: &html5gum::StartTag<()>) {
        let address = tag
            .attributes
            .get(b"href".as_slice())
            .and_then(|href| self.link_url(&href.value));
        if let Some(table) = &mut self.table {
            table.begin_link(address);
        } else {
            self.close_link();
            if address.is_some() {
                self.text.push('[');
            }
            self.link = address;
        }
    }

    fn begin_code(&mut self, tag: &html5gum::StartTag<()>) {
        self.finish_code();
        self.close_link();
        self.paragraph();
        self.preformatted = Some(CodeBlock::new(tag));
    }

    fn finish_code(&mut self) {
        if let Some(block) = self.preformatted.take() {
            self.text.push_str(&block.markdown());
        }
    }

    fn link_url(&self, href: &[u8]) -> Option<String> {
        let href = String::from_utf8_lossy(href);
        let url = self
            .base
            .as_ref()
            .and_then(|base| base.join(&href).ok())
            .or_else(|| url::Url::parse(&href).ok())?;
        crate::source::link_url(url.as_str())
            .ok()
            .map(|url| url.to_string())
    }

    fn tag(&mut self, name: &str, closing: bool) {
        if name == "pre" {
            self.finish_code();
            return;
        }
        if self.preformatted.is_some() {
            return;
        }
        match name {
            "p" | "div" | "section" | "ul" | "ol" => self.paragraph(),
            "blockquote" => {
                self.paragraph();
                self.quotes = if closing {
                    self.quotes.saturating_sub(1)
                } else {
                    self.quotes + 1
                };
            }
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.paragraph();
                if !closing {
                    self.text.push_str(
                        &"#".repeat(usize::from(name.as_bytes()[1] - b'0')),
                    );
                    self.text.push(' ');
                }
            }
            "strong" | "b" => self.text.push_str("**"),
            "em" | "i" => self.text.push('*'),
            "code" => self.text.push(char::from(96)),
            "br" | "tr" => self.text.push('\n'),
            "li" if !closing => self.text.push_str("\n- "),
            "td" | "th" if closing => self.text.push_str("  "),
            "a" if closing => self.close_link(),
            _ => {}
        }
    }

    fn close_link(&mut self) {
        if let Some(link) = self.link.take() {
            self.text.push_str("](<");
            self.text.push_str(&link);
            self.text.push_str(">)");
        }
    }

    fn append(&mut self, text: &str) {
        if let Some(table) = &mut self.table {
            table.text(&clean(text));
            return;
        }
        if let Some(block) = &mut self.preformatted {
            block.text.push_str(&clean(text));
            return;
        }
        for character in clean(text).chars() {
            if character.is_whitespace() {
                if !self.text.ends_with(char::is_whitespace) {
                    self.text.push(' ');
                }
            } else {
                if self.quotes > 0
                    && (self.text.is_empty() || self.text.ends_with('\n'))
                {
                    self.text.push_str("> ");
                }
                self.text.push(character);
            }
        }
    }

    fn paragraph(&mut self) {
        self.text.truncate(self.text.trim_end().len());
        if !self.text.is_empty() {
            self.text.push_str("\n\n");
        }
    }

    fn finish(mut self) -> String {
        if let Some(table) = self.table.take() {
            self.text.push_str(&table.markdown());
        }
        self.close_link();
        self.finish_code();
        clean(self.text.trim())
    }

    fn table_tag(&mut self, name: &str, closing: bool) {
        if name == "table" && self.table.is_none() && !closing {
            self.close_link();
            self.paragraph();
            self.table = Some(table::Table::default());
            return;
        }
        if name == "table"
            && closing
            && self.table.as_ref().is_some_and(|table| !table.nested())
        {
            if let Some(table) = self.table.take() {
                self.text.push_str(&table.markdown());
                self.paragraph();
            }
        } else if let Some(table) = &mut self.table {
            table.tag(name, closing);
        }
    }
}
