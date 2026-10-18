//! Bounded article-page fetching and library-owned Readability extraction.
//!
//! One dedicated worker serves opened articles independently of feed refreshes.
//! It never executes JavaScript or fetches linked assets. Extracted HTML passes
//! through the same terminal-safe normalizer as feeds, retaining code metadata.

pub mod rules;
mod worker;

pub use worker::{Completion, Worker};

use anyhow::{Context, Result, ensure};
use dom_smoothie::{Config, Readability};
use ureq::ResponseExt;

use crate::{content, model::ExtractedDocument};

const MAX_PAGE_BYTES: usize = 2 * 1024 * 1024;

/// Fetch/cache one existing article through the same path as the TUI worker.
///
/// # Errors
/// Returns missing/stale targets, HTTP, extraction or storage failures. A failed
/// fetch records its reason while preserving any previous successful cache.
pub fn fetch_once(path: &std::path::Path, article: i64) -> Result<()> {
    worker::collect_once(path, article)
}

/// Extract readable article HTML into bounded Markdown without network access.
///
/// # Errors
/// Returns invalid URLs, oversized/complex pages or unreadable-content errors.
pub fn extract(html: &str, address: &str) -> Result<ExtractedDocument> {
    extract_with_selector(html, address, "")
}

/// Extract using Readability or one explicitly chosen CSS container.
///
/// # Errors
/// Returns invalid selectors, ambiguous matches or ordinary extraction errors.
pub fn extract_with_selector(
    html: &str,
    address: &str,
    selector: &str,
) -> Result<ExtractedDocument> {
    ensure!(html.len() <= MAX_PAGE_BYTES, "Article page exceeds 2 MiB");
    let address = super::link_url(address)?;
    let fragment = if selector.is_empty() {
        readable_html(html, address.as_str())?
    } else {
        rules::select(html, selector)?
    };
    normalize_fragment(&fragment, address.as_str())
}

fn readable_html(html: &str, address: &str) -> Result<String> {
    let config = Config {
        keep_classes: true,
        max_elements_to_parse: 20_000,
        disable_json_ld: true,
        ..Config::default()
    };
    let mut reader = Readability::new(html, Some(address), Some(config))
        .context("Could not parse the article page")?;
    let article = reader
        .parse()
        .context("No readable article found; the page may require a browser")?;
    Ok(article.content.to_string())
}

fn normalize_fragment(
    fragment: &str,
    address: &str,
) -> Result<ExtractedDocument> {
    ensure!(
        fragment.len() <= 262_144,
        "Article fragment exceeds 256 KiB"
    );
    let body = content::article_markdown(fragment, true, address);
    ensure!(body.len() <= 524_288, "Extracted article exceeds 512 KiB");
    ensure!(
        content::body_kind(&body) == content::BodyKind::Text,
        "No readable article text was extracted; use the original in a browser"
    );
    rules::reject_obstruction(&body)?;
    Ok(ExtractedDocument {
        final_url: address.to_string(),
        body,
    })
}

pub(super) fn fetch_with_selector(
    client: &ureq::Agent,
    address: &str,
    selector: &str,
) -> Result<ExtractedDocument> {
    let url = super::link_url(address)?;
    let mut response = client
        .get(url.as_str())
        .header("Accept", "text/html, application/xhtml+xml;q=0.9")
        .call()
        .context("Could not download the original article")?;
    ensure!(
        response.status() == 200,
        "Article returned HTTP {}",
        response.status()
    );
    let mime = response
        .headers()
        .get("content-type")
        .and_then(|header| header.to_str().ok())
        .unwrap_or_default();
    let mime = mime.split(';').next().unwrap_or_default().trim();
    ensure!(
        mime.eq_ignore_ascii_case("text/html")
            || mime.eq_ignore_ascii_case("application/xhtml+xml"),
        "Original is not an HTML article; open it in a browser"
    );
    let address = response.get_uri().to_string();
    let bytes = response
        .body_mut()
        .with_config()
        .limit(MAX_PAGE_BYTES as u64)
        .read_to_vec()?;
    let html = std::str::from_utf8(&bytes)
        .context("Article encoding is not UTF-8; open it in a browser")?;
    extract_with_selector(html, &address, selector)
}
