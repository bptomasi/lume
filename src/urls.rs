//! Bounded HTTP(S) addresses shared by content, configuration and adapters.
//!
//! These functions normalize or validate addresses without network access.

use anyhow::{Context, Result, ensure};
use url::Url;

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

/// Normalize an explicit subscription URL without guessing discovery endpoints.
///
/// # Errors
/// Rejects relative URLs, credentials and schemes other than HTTP(S).
pub fn feed_url(address: &str) -> Result<Url> {
    let mut url = link_url(address)?;
    url.set_fragment(None);
    Ok(url)
}

/// Normalize a remote server root so API paths can be appended directly.
///
/// # Errors
/// Rejects credentials, queries, fragments and non-HTTP(S) schemes.
pub fn server_url(address: &str) -> Result<String> {
    let mut url = link_url(address)?;
    ensure!(
        url.query().is_none() && url.fragment().is_none(),
        "Server URL cannot contain a query or fragment"
    );
    let path = url.path().trim_end_matches('/').to_owned();
    url.set_path(&path);
    Ok(url.as_str().trim_end_matches('/').to_owned())
}

/// Validate a browser link while preserving an article's fragment anchor.
///
/// # Errors
/// Returns invalid URL, credentials or unsupported scheme errors.
pub fn link_url(address: &str) -> Result<Url> {
    ensure!(address.len() <= 8192, "Feed URL exceeds 8 KiB");
    let url = Url::parse(address.trim()).context("Enter a full feed URL")?;
    validate_url(url)
}

/// Resolve a publisher link against its page, retaining queries and fragments.
///
/// # Errors
/// Rejects invalid destinations, credentials and non-HTTP(S) schemes.
pub fn resolve_link(base: &str, destination: &str) -> Result<Url> {
    ensure!(destination.len() <= 8192, "Link URL exceeds 8 KiB");
    ensure!(
        !destination.is_empty() && !destination.chars().any(char::is_control),
        "Invalid link destination"
    );
    let url = match Url::parse(destination) {
        Ok(url) => url,
        Err(url::ParseError::RelativeUrlWithoutBase) => {
            link_url(base)?.join(destination)?
        }
        Err(error) => return Err(error.into()),
    };
    validate_url(url)
}

fn validate_url(url: Url) -> Result<Url> {
    ensure!(
        matches!(url.scheme(), "http" | "https"),
        "Use HTTP or HTTPS"
    );
    ensure!(url.host_str().is_some(), "Feed URL needs a host");
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "Credentials in feed URLs are not supported"
    );
    Ok(url)
}
