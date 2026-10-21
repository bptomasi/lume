//! Feed discovery from HTML link metadata, with HTML5 attribute recovery.

use std::collections::BTreeSet;

use anyhow::{Result, ensure};
use html5gum::{DefaultEmitter, StartTag, Token, Tokenizer};
use ureq::ResponseExt;

/// An explicit discovered subscription. Discovery never subscribes silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Publisher label, or normalized URL when absent.
    pub title: String,
    /// Validated absolute HTTP(S) URL.
    pub url: String,
}

/// Extract advertised RSS/Atom/JSON feeds relative to a page's final URL.
///
/// # Errors
/// Returns invalid base URL or oversized HTML errors.
pub fn links(html: &str, address: &str) -> Result<Vec<Candidate>> {
    ensure!(html.len() <= 4 * 1024 * 1024, "Page exceeds 4 MiB");
    let mut base = super::feed_url(address)?;
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    let mut emitter = DefaultEmitter::default();
    emitter.naively_switch_states(true);
    let mut seen_base = false;
    for token in Tokenizer::new_with_emitter(html, emitter).flatten() {
        let Token::StartTag(tag) = token else {
            continue;
        };
        if tag.name.as_ref() == b"base" && !seen_base {
            seen_base = true;
            if let Some(url) =
                attr(&tag, b"href").and_then(|href| base.join(&href).ok())
                && let Ok(valid) = super::feed_url(url.as_str())
            {
                base = valid;
            }
        }
        if let Some(candidate) = candidate(&tag, &base)
            && seen.insert(candidate.url.clone())
        {
            found.push(candidate);
        }
        ensure!(found.len() <= 100, "Page advertises more than 100 feeds");
    }
    Ok(found)
}

/// Fetch an explicit page/feed URL and return choices for user selection.
///
/// # Errors
/// Returns HTTP, size, feed-decoding or discovery errors.
pub fn discover(address: &str) -> Result<Vec<Candidate>> {
    let url = super::feed_url(address)?;
    let mut response = super::agent().get(url.as_str()).call()?;
    ensure!(
        response.status() == 200,
        "Discovery returned HTTP {}",
        response.status()
    );
    let final_url = response.get_uri().to_string();
    let bytes = super::download::read_feed(response.body_mut())?;
    if let Ok(feed) = super::parse(&bytes, &final_url) {
        return Ok(vec![Candidate {
            title: feed.title,
            url: final_url,
        }]);
    }
    let candidates = links(&String::from_utf8_lossy(&bytes), &final_url)?;
    ensure!(
        !candidates.is_empty(),
        "No advertised feeds found; enter an explicit feed URL"
    );
    Ok(candidates)
}

fn candidate(tag: &StartTag<()>, base: &url::Url) -> Option<Candidate> {
    if tag.name.as_ref() != b"link" {
        return None;
    }
    let alternate = attr(tag, b"rel")?
        .split_ascii_whitespace()
        .any(|rel| rel.eq_ignore_ascii_case("alternate"));
    let mime = attr(tag, b"type")?.to_ascii_lowercase();
    if !alternate
        || !matches!(
            mime.split(';').next()?.trim(),
            "application/rss+xml"
                | "application/atom+xml"
                | "application/feed+json"
        )
    {
        return None;
    }
    let url =
        super::feed_url(base.join(&attr(tag, b"href")?).ok()?.as_str()).ok()?;
    Some(Candidate {
        title: attr(tag, b"title").map_or_else(
            || url.to_string(),
            |title| crate::content::label(&title, 200),
        ),
        url: url.to_string(),
    })
}

pub(crate) fn attr(tag: &StartTag<()>, key: &[u8]) -> Option<String> {
    tag.attributes
        .get(key)
        .map(|value| String::from_utf8_lossy(&value.value).into_owned())
}
