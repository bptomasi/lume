//! HTML anchor identity survives normalization without becoming visible text.

use html5gum::{Token, Tokenizer};

pub(crate) fn identifier(tag: &html5gum::StartTag<()>) -> Option<String> {
    let value = tag.attributes.get(b"id".as_slice()).or_else(|| {
        if tag.name.as_slice() != b"a" {
            return None;
        }
        tag.attributes.get(b"name".as_slice())
    })?;
    let value = std::str::from_utf8(&value.value).ok()?;
    (!value.is_empty()
        && value.len() <= 1024
        && !value.chars().any(char::is_control))
    .then(|| value.to_owned())
}

pub(crate) fn marker(tag: &html5gum::StartTag<()>) -> Option<String> {
    let id = identifier(tag)?;
    let escaped = id
        .replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    Some(format!("<a id=\"{escaped}\"></a>"))
}

pub(crate) fn from_html(html: &str) -> Option<String> {
    if html.len() > 8192 || !html.trim_start().starts_with("<a ") {
        return None;
    }
    Tokenizer::new(html).flatten().find_map(|token| {
        let Token::StartTag(tag) = token else {
            return None;
        };
        identifier(&tag)
    })
}

pub(crate) fn invisible(html: &str) -> bool {
    html.trim() == "</a>" || from_html(html).is_some()
}
