//! Publisher code metadata and literal, collision-free Markdown fences.

use html5gum::StartTag;

use crate::source::discovery::attr;

pub(super) struct CodeBlock {
    pub language: Option<String>,
    pub text: String,
}

impl CodeBlock {
    pub fn new(tag: &StartTag<()>) -> Self {
        Self {
            language: language(tag),
            text: String::new(),
        }
    }

    pub fn set_language(&mut self, tag: &StartTag<()>) {
        if let Some(language) = language(tag) {
            self.language = Some(language);
        }
    }

    pub fn markdown(self) -> String {
        // A publisher's literal fence must never terminate our code block.
        let longest = self
            .text
            .split(|c| c != '~')
            .map(str::len)
            .max()
            .unwrap_or(0);
        let fence = "~".repeat(longest.saturating_add(1).max(4));
        let newline = if self.text.ends_with('\n') { "" } else { "\n" };
        format!(
            "{fence}{}\n{}{newline}{fence}\n",
            self.language.as_deref().unwrap_or_default(),
            self.text,
        )
    }
}

fn language(tag: &StartTag<()>) -> Option<String> {
    for key in [b"data-language".as_slice(), b"data-lang", b"lang"] {
        if let Some(value) = attr(tag, key)
            && valid_token(&value)
        {
            return Some(value);
        }
    }
    let classes = attr(tag, b"class")?;
    classes.split_ascii_whitespace().find_map(|class| {
        let token = ["language-", "lang-", "highlight-source-"]
            .iter()
            .find_map(|prefix| class.strip_prefix(prefix))?;
        valid_token(token).then(|| token.to_owned())
    })
}

fn valid_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 32
        && token.chars().all(|c| {
            c.is_ascii_alphanumeric()
                || matches!(c, '+' | '#' | '-' | '_' | '.')
        })
}
