//! Independent feed collection using ureq and feed-rs.
//!
//! HTTP, parsing and ingestion run on one worker. The UI submits at most one
//! refresh at a time; progress crosses a bounded channel. Each feed commits
//! independently, so an unavailable publisher does not discard other results.

pub mod article;
mod body;
pub mod discovery;
mod download;
mod scheduler;
mod worker;

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, ensure};
use feed_rs::{model::Entry, parser};
use ureq::tls::{Certificate, RootCerts, TlsConfig};
use url::Url;

use crate::{content, model::IncomingArticle};

pub use worker::{Progress, Worker};

/// Group exact article URLs after removing fragments and known tracking keys.
///
/// Original URLs remain untouched. Other query parameters retain their order
/// and values; similar titles alone never establish duplicate identity.
pub fn canonical_article_url(address: &str) -> String {
    let Ok(mut url) = link_url(address) else {
        return String::new();
    };
    url.set_fragment(None);
    let parameters = url
        .query_pairs()
        .filter(|(name, _)| {
            !name.starts_with("utm_")
                && !matches!(
                    name.as_ref(),
                    "fbclid" | "gclid" | "mc_cid" | "mc_eid"
                )
        })
        .map(|(name, value)| (name.into_owned(), value.into_owned()))
        .collect::<Vec<_>>();
    url.set_query(None);
    if !parameters.is_empty() {
        url.query_pairs_mut().extend_pairs(parameters);
    }
    url.to_string()
}

/// Collect subscriptions for a non-interactive CLI or real-service test.
///
/// # Errors
/// Returns database errors. Individual feed failures are saved and counted.
pub fn collect_once(path: &std::path::Path) -> Result<(usize, usize)> {
    worker::collect_once(path)
}

/// A normalized feed ready for storage; publisher HTML is already converted.
pub struct ParsedFeed {
    /// Source title.
    pub title: String,
    /// At most 2,000 articles.
    pub articles: Vec<IncomingArticle>,
}

/// Normalize an explicit subscription URL without guessing discovery endpoints.
///
/// # Errors
/// Rejects relative URLs, credentials and schemes other than HTTP(S).
pub fn feed_url(address: &str) -> Result<Url> {
    ensure!(address.len() <= 8192, "Feed URL exceeds 8 KiB");
    let mut url =
        Url::parse(address.trim()).context("Enter a full feed URL")?;
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Use HTTP or HTTPS"
    );
    ensure!(url.host_str().is_some(), "Feed URL needs a host");
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Credentials in feed URLs are not supported"
    );
    url.set_fragment(None);
    Ok(url)
}

/// Validate a browser link while preserving an article's fragment anchor.
///
/// # Errors
/// Returns invalid URL, credentials or unsupported scheme errors.
pub fn link_url(address: &str) -> Result<Url> {
    let mut url = feed_url(address)?;
    let original = Url::parse(address.trim())?;
    url.set_fragment(original.fragment());
    Ok(url)
}

/// Decode RSS, Atom or JSON Feed with publisher-specific details kept here.
///
/// # Errors
/// Rejects malformed feeds, bodies over 4 MiB and feeds over 2,000 entries.
pub fn parse(bytes: &[u8], address: &str) -> Result<ParsedFeed> {
    ensure!(bytes.len() <= 4 * 1024 * 1024, "Feed exceeds 4 MiB");
    let feed = parser::Builder::new()
        .base_uri(Some(address))
        .id_generator(fallback_id)
        .build()
        .parse(bytes)
        .context("Publisher did not return a supported feed")?;
    ensure!(feed.entries.len() <= 2000, "Feed exceeds 2,000 entries");
    ensure!(
        feed.entries
            .iter()
            .all(|entry| !entry.id.is_empty() && entry.id.len() <= 4096),
        "An entry needs an ID, link or title within 4 KiB"
    );
    let title = feed.title.map_or_else(
        || address.to_owned(),
        |title| content::label(&title.content, 200),
    );
    let articles = feed.entries.into_iter().map(normalize).collect();
    Ok(ParsedFeed { title, articles })
}

fn fallback_id(
    links: &[feed_rs::model::Link],
    title: &Option<feed_rs::model::Text>,
    _base: Option<&str>,
) -> String {
    if let Some(link) = links
        .iter()
        .find(|link| link.rel.as_deref().is_none_or(|rel| rel == "alternate"))
    {
        return format!("link:{}", link.href);
    }
    // The database scopes this fallback to its feed. A publisher without IDs
    // or links makes title changes indistinguishable from a new article.
    title
        .as_ref()
        .map_or_else(String::new, |title| format!("title:{}", title.content))
}

pub(crate) fn agent() -> ureq::Agent {
    let roots = rustls_native_certs::load_native_certs()
        .certs
        .iter()
        .map(|der| Certificate::from_der(der.as_ref()).to_owned())
        .collect::<Vec<_>>();
    let tls = TlsConfig::builder()
        .root_certs(RootCerts::from(roots))
        .unversioned_rustls_crypto_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .timeout_global(Some(Duration::from_secs(12)))
        .max_redirects(5)
        .user_agent("Lume/0.2 (independent feed reader)")
        .build()
        .into()
}

fn normalize(entry: Entry) -> IncomingArticle {
    let url = entry
        .links
        .iter()
        .filter(|link| link.rel.as_deref().is_none_or(|rel| rel == "alternate"))
        .find_map(|link| link_url(&link.href).ok())
        .map_or_else(String::new, |url| url.to_string());
    let title = entry.title.as_ref().map_or_else(
        || "Untitled article".to_owned(),
        |title| content::label(&title.content, 500),
    );
    let author = entry
        .authors
        .iter()
        .filter_map(|person| person.name.as_deref())
        .map(|name| content::label(name, 100))
        .collect::<Vec<_>>()
        .join(", ");
    let (body, body_origin) = body::normalize(&entry, &url);
    IncomingArticle {
        external_id: entry.id.clone(),
        title,
        url,
        author,
        published: entry
            .published
            .or(entry.updated)
            .map_or(0, |date| date.timestamp()),
        body,
        body_origin,
    }
}
