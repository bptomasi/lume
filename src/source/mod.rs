//! Independent feed collection using ureq and feed-rs.
//!
//! HTTP and parsing run on bounded lanes; one collector owns SQLite writes.
//! The UI submits at most one refresh at a time and receives bounded progress.
//! Each feed commits independently, so an unavailable publisher does not
//! discard other results.

pub mod article;
mod body;
pub mod discovery;
mod download;
mod miniflux;
mod scheduler;
mod worker;

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, ensure};
use feed_rs::{model::Entry, parser};
use ureq::tls::{Certificate, RootCerts, TlsConfig};

use crate::{content, model::IncomingArticle};

pub use crate::urls::{
    canonical_article_url, feed_url, link_url, resolve_link, server_url,
};
pub use worker::{Progress, Worker};

/// Maximum decompressed RSS, Atom or JSON Feed document size: 16 MiB.
///
/// Full-history feeds can exceed 4 MiB. Collection and direct parsing share
/// this bound; the collector stops reading after one extra byte.
pub const MAX_FEED_BYTES: usize = 16 * 1024 * 1024;

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
    /// Optional server-owned batch identity and first-observation state.
    pub remote: Option<crate::model::RemoteFeedUpdate>,
}

/// Decode RSS, Atom or JSON Feed with publisher-specific details kept here.
///
/// # Errors
/// Rejects malformed feeds, bodies over 16 MiB and feeds over 2,000 entries.
pub fn parse(bytes: &[u8], address: &str) -> Result<ParsedFeed> {
    validate_feed_size(bytes.len())?;
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
    Ok(ParsedFeed {
        title,
        articles,
        remote: None,
    })
}

pub(super) fn validate_feed_size(bytes: usize) -> Result<()> {
    ensure!(bytes <= MAX_FEED_BYTES, "Feed exceeds 16 MiB");
    Ok(())
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
