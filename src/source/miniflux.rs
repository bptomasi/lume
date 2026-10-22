//! Optional Miniflux mirror with bounded discovery and entry pages.
//!
//! The server owns subscription collection; Lume caches entries and keeps
//! personal read/starred state. No request runs on the input or render lanes.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, bail, ensure};
use serde::de::DeserializeOwned;

use crate::{
    content,
    model::{
        Feed, FeedOrigin, IncomingArticle, RemoteArticleState,
        RemoteFeedCandidate, RemoteFeedUpdate, Server, ServerFeed,
    },
    store::Store,
};

use super::{MAX_FEED_BYTES, ParsedFeed, download::Download, link_url};

/// Entries requested per page. Keeps one decoded JSON body below 16 MiB for
/// ordinary sanitized content.
const PAGE_ENTRIES: usize = 200;
/// Pages visited per feed and sync. Also bounds a first observation to 2,000
/// entries, matching the ordinary feed contract.
const MAX_PAGES: usize = 10;
/// Largest normalized remote body accepted for one entry.
const MAX_REMOTE_BODY: usize = 1024 * 1024;

/// Discover remote subscriptions for every enabled server.
///
/// Returns the number of servers whose discovery failed. Feed-level failures
/// remain in the ordinary source health records.
pub(super) fn sync_servers(
    store: &mut Store,
    client: &ureq::Agent,
    stopped: &AtomicBool,
) -> Result<usize> {
    let servers = store.servers()?;
    let mut failures = 0;
    for server in servers.into_iter().filter(|server| server.enabled) {
        if stopped.load(Ordering::Relaxed) {
            break;
        }
        let result = sync_server(store, client, &server);
        if let Err(error) = &result {
            failures += 1;
            store.record_server_attempt(
                server.id,
                now(),
                &format!("{error:#}"),
            )?;
        } else {
            store.record_server_attempt(server.id, now(), "")?;
        }
    }
    Ok(failures)
}

fn sync_server(
    store: &mut Store,
    client: &ureq::Agent,
    server: &Server,
) -> Result<()> {
    let token = token(server)?;
    let feeds: Vec<RemoteFeed> =
        get_json(client, &endpoint(server, "/v1/feeds")?, &token)
            .map_err(|error| annotate(error, &server.token_env))?;
    ensure!(feeds.len() <= 5000, "Server exceeds 5,000 feeds");
    let candidates = feeds
        .into_iter()
        .map(|feed| {
            ensure!(feed.id > 0, "Remote feed has an invalid ID");
            let local_url = format!("{}/v1/feeds/{}", server.url, feed.id);
            let remote_url = super::feed_url(&feed.feed_url)
                .map_or_else(|_| String::new(), |url| url.to_string());
            Ok(RemoteFeedCandidate {
                remote_id: feed.id,
                title: content::label(&feed.title, 200),
                remote_url,
                local_url,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    store.sync_server_feeds(server.id, &candidates)
}

/// Fetch one bounded entry page set for a mirrored feed.
pub(super) fn fetch_entries(
    client: &ureq::Agent,
    feed: &Feed,
    server: &ServerFeed,
) -> Download {
    let result = request_entries(client, feed, server);
    Download {
        feed: feed.clone(),
        result,
        retry_at: 0,
    }
}

fn request_entries(
    client: &ureq::Agent,
    feed: &Feed,
    server: &ServerFeed,
) -> Result<Option<ParsedFeed>> {
    let token = token_from(&server.token_env)?;
    let entries = fetch_pages(client, server, &token)?;
    let (articles, states, last_changed) = normalize_entries(server, entries)?;
    let title = content::label(&feed.title, 200);
    Ok(Some(ParsedFeed {
        title,
        articles,
        remote: Some(RemoteFeedUpdate {
            server_id: server.server_id,
            remote_id: server.remote_id,
            initialized: true,
            last_changed,
            states,
        }),
    }))
}

fn fetch_pages(
    client: &ureq::Agent,
    server: &ServerFeed,
    token: &str,
) -> Result<BTreeMap<i64, RemoteEntry>> {
    let initial = !server.initialized;
    let mut offset = 0;
    let mut pages = 0;
    let mut entries = BTreeMap::new();
    loop {
        if entries.len() >= MAX_PAGES * PAGE_ENTRIES || pages >= MAX_PAGES {
            break;
        }
        pages += 1;
        let url = entry_page_url(server, initial, offset)?;
        let page: EntryPage = get_json(client, &url, token)
            .map_err(|error| annotate(error, &server.token_env))?;
        ensure!(page.entries.len() <= PAGE_ENTRIES, "Oversized entry page");
        let complete = page.entries.len() < PAGE_ENTRIES
            || offset + page.entries.len() >= page.total;
        for entry in page.entries {
            entries.entry(entry.id).or_insert(entry);
        }
        offset += PAGE_ENTRIES;
        if complete || entries.len() >= MAX_PAGES * PAGE_ENTRIES {
            break;
        }
    }
    Ok(entries)
}

fn entry_page_url(
    server: &ServerFeed,
    initial: bool,
    offset: usize,
) -> Result<url::Url> {
    let mut url = endpoint_parts(
        &server.server_url,
        &format!("/v1/feeds/{}/entries", server.remote_id),
    )?;
    {
        let mut query = url.query_pairs_mut();
        query
            .append_pair("limit", &PAGE_ENTRIES.to_string())
            .append_pair("offset", &offset.to_string());
        if initial {
            query
                .append_pair("order", "published_at")
                .append_pair("direction", "desc");
        } else {
            query
                .append_pair("order", "changed_at")
                .append_pair("direction", "asc")
                .append_pair(
                    "changed_after",
                    &server.last_changed.saturating_sub(1).to_string(),
                );
        }
    }
    Ok(url)
}

fn normalize_entries(
    server: &ServerFeed,
    entries: BTreeMap<i64, RemoteEntry>,
) -> Result<(
    Vec<IncomingArticle>,
    BTreeMap<String, RemoteArticleState>,
    i64,
)> {
    let mut states = BTreeMap::new();
    let mut articles = Vec::with_capacity(entries.len());
    let mut last_changed = server.last_changed;
    for entry in entries.into_values() {
        if !matches!(entry.status.as_str(), "read" | "unread") {
            continue;
        }
        last_changed = last_changed.max(timestamp(&entry.changed_at));
        let external_id = format!("miniflux:{}", entry.id);
        states.insert(
            external_id.clone(),
            RemoteArticleState {
                unread: entry.status == "unread",
                starred: entry.starred,
            },
        );
        articles.push(normalize(entry, external_id)?);
    }
    Ok((articles, states, last_changed))
}

fn normalize(
    entry: RemoteEntry,
    external_id: String,
) -> Result<IncomingArticle> {
    let url = link_url(&entry.url)
        .map_or_else(|_| String::new(), |url| url.to_string());
    let title = if entry.title.trim().is_empty() {
        "Untitled article".to_owned()
    } else {
        content::label(&entry.title, 500)
    };
    let author = content::label(&entry.author, 100);
    let (body, body_origin) = if entry.content.trim().is_empty() {
        (content::MISSING_BODY.to_owned(), FeedOrigin::Missing)
    } else {
        let body = content::article_markdown(&entry.content, true, &url);
        ensure!(body.len() <= MAX_REMOTE_BODY, "Remote entry exceeds 1 MiB");
        (body, FeedOrigin::Feed)
    };
    Ok(IncomingArticle {
        external_id,
        title,
        url,
        author,
        published: timestamp(&entry.published_at),
        body,
        body_origin,
    })
}

fn token(server: &Server) -> Result<String> {
    token_from(&server.token_env)
}

fn token_from(name: &str) -> Result<String> {
    let value = std::env::var(name)
        .with_context(|| format!("Set {name} to the Miniflux API token"))?;
    ensure!(
        !value.trim().is_empty()
            && value.len() <= 4096
            && !value.chars().any(char::is_control),
        "Miniflux token environment variable is empty or invalid"
    );
    Ok(value)
}

fn endpoint(server: &Server, path: &str) -> Result<url::Url> {
    endpoint_parts(&server.url, path)
}

fn endpoint_parts(root: &str, path: &str) -> Result<url::Url> {
    url::Url::parse(&format!("{root}{path}"))
        .with_context(|| format!("Invalid Miniflux endpoint for {root}"))
}

fn get_json<T: DeserializeOwned>(
    client: &ureq::Agent,
    url: &url::Url,
    token: &str,
) -> Result<T> {
    let mut response = client
        .get(url.as_str())
        .header("Accept", "application/json")
        .header("X-Auth-Token", token)
        .config()
        .http_status_as_error(false)
        .build()
        .call()?;
    if response.status() != 200 {
        bail!("Miniflux returned HTTP {}", response.status());
    }
    let bytes = crate::http_body::read(response.body_mut(), MAX_FEED_BYTES)
        .context("Miniflux response exceeds 16 MiB")?;
    serde_json::from_slice(&bytes).context("Invalid Miniflux JSON response")
}

fn annotate(error: anyhow::Error, token_env: &str) -> anyhow::Error {
    let text = format!("{error:#}");
    if text.contains("HTTP 401") || text.contains("HTTP 403") {
        anyhow::anyhow!("Miniflux authentication failed; check {token_env}")
    } else {
        error
    }
}

fn timestamp(value: &str) -> i64 {
    chrono::DateTime::parse_from_rfc3339(value)
        .map_or(0, |date| date.timestamp())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

#[derive(serde::Deserialize)]
struct RemoteFeed {
    id: i64,
    title: String,
    feed_url: String,
}

#[derive(serde::Deserialize)]
struct EntryPage {
    #[serde(default)]
    total: usize,
    entries: Vec<RemoteEntry>,
}

#[derive(serde::Deserialize)]
struct RemoteEntry {
    id: i64,
    title: String,
    url: String,
    author: String,
    content: String,
    published_at: String,
    changed_at: String,
    status: String,
    starred: bool,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use anyhow::Result;

    use super::*;

    #[test]
    fn initial_and_incremental_page_urls_are_bounded_and_explicit() -> Result<()>
    {
        let initial_server = server(false, 0);
        let initial = entry_page_url(&initial_server, true, 0)?;
        let query = parameters(&initial);
        assert_eq!(query.get("limit").map(String::as_str), Some("200"));
        assert_eq!(query.get("offset").map(String::as_str), Some("0"));
        assert_eq!(
            query.get("order").map(String::as_str),
            Some("published_at")
        );
        assert_eq!(query.get("direction").map(String::as_str), Some("desc"));
        assert!(!query.contains_key("changed_after"));

        let incremental = entry_page_url(&server(true, 100), false, 200)?;
        let query = parameters(&incremental);
        assert_eq!(query.get("offset").map(String::as_str), Some("200"));
        assert_eq!(query.get("order").map(String::as_str), Some("changed_at"));
        assert_eq!(query.get("direction").map(String::as_str), Some("asc"));
        assert_eq!(query.get("changed_after").map(String::as_str), Some("99"));
        Ok(())
    }

    #[test]
    fn timestamps_accept_rfc3339_and_reject_invalid_values() {
        assert_eq!(timestamp("2026-10-01T10:00:00Z"), 1_790_848_800);
        assert_eq!(timestamp("not-a-date"), 0);
    }

    fn server(initialized: bool, last_changed: i64) -> ServerFeed {
        ServerFeed {
            server_id: 1,
            server_url: "https://reader.example".into(),
            token_env: "LUME_MINIFLUX_TOKEN".into(),
            enabled: true,
            remote_id: 7,
            initialized,
            last_changed,
            present: true,
        }
    }

    fn parameters(url: &url::Url) -> BTreeMap<String, String> {
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect()
    }
}
