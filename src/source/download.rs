//! HTTP downloads produce owned results; only the collector commits SQLite.

use std::io::Read;

use anyhow::{Context, Result, bail};

use crate::model::Feed;

use super::ParsedFeed;

pub(super) struct Download {
    pub(super) feed: Feed,
    pub(super) result: Result<Option<ParsedFeed>>,
    pub(super) retry_at: i64,
}

pub(super) fn fetch(client: &ureq::Agent, feed: &Feed) -> Download {
    let mut updated = feed.clone();
    let mut retry_at = 0;
    let result = request(client, feed).and_then(|mut response| {
        if response.status() == 304 {
            return Ok(None);
        }
        if response.status() != 200 {
            retry_at = retry_after(
                &header(&response, "retry-after"),
                chrono::Utc::now().timestamp(),
            )
            .unwrap_or(0);
            bail!("Unexpected HTTP {}", response.status());
        }
        updated.etag = header(&response, "etag");
        updated.modified = header(&response, "last-modified");
        let bytes = read_feed(response.body_mut())?;
        super::parse(&bytes, &feed.url).map(Some)
    });
    Download {
        feed: updated,
        result,
        retry_at,
    }
}

pub(super) fn read_feed(body: &mut ureq::Body) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    // ureq's configured limit wraps the wire stream before gzip decoding.
    // Bound its decoded reader instead, including the one-byte overflow probe.
    body.as_reader()
        .take((super::MAX_FEED_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .context("Could not read feed response")?;
    super::validate_feed_size(bytes.len())?;
    Ok(bytes)
}

fn request(
    client: &ureq::Agent,
    feed: &Feed,
) -> Result<ureq::http::Response<ureq::Body>> {
    // Preserve this exact HTTP Accept value as a wire contract.
    let mut request = client.get(&feed.url)
        .header("Accept", "application/atom+xml, application/rss+xml, application/feed+json, application/xml, text/xml");
    if !feed.etag.is_empty() {
        request = request.header("If-None-Match", &feed.etag);
    }
    if !feed.modified.is_empty() {
        request = request.header("If-Modified-Since", &feed.modified);
    }
    Ok(request
        .config()
        .http_status_as_error(false)
        .build()
        .call()?)
}

/// Interpret HTTP Retry-After seconds or an HTTP date; expired dates mean now.
pub(super) fn retry_after(value: &str, now: i64) -> Option<i64> {
    if let Ok(seconds) = value.trim().parse::<u64>() {
        return Some(
            now.saturating_add(i64::try_from(seconds).unwrap_or(i64::MAX)),
        );
    }
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|date| date.timestamp().max(now))
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .chars()
        .take(1000)
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn retry_after_handles_seconds_dates_expiry_and_invalid_values() {
        assert_eq!(super::retry_after("120", 100), Some(220));
        assert_eq!(
            super::retry_after("Sun, 06 Nov 1994 08:49:37 GMT", 0),
            Some(784_111_777)
        );
        assert_eq!(
            super::retry_after("Sun, 06 Nov 1994 08:49:37 GMT", 900_000_000),
            Some(900_000_000)
        );
        assert_eq!(super::retry_after("invalid", 100), None);
        assert_eq!(super::retry_after("-1", 100), None);
    }
}
