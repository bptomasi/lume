//! Optional Miniflux mirror with bounded discovery and entry pages.
//!
//! The server owns subscription collection; Lume caches entries and keeps
//! personal read/starred state. No request runs on the input or render lanes.

use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

use anyhow::{Context, Result, ensure};
use serde::de::DeserializeOwned;

use crate::{
    content,
    model::{
        Feed, FeedOrigin, IncomingArticle, RemoteArticleState,
        RemoteFeedCandidate, RemoteFeedUpdate, Server, ServerFeed,
    },
};

use super::{MAX_FEED_BYTES, ParsedFeed, download::Download, link_url};

mod pagination;
use pagination::{Collected, collect_pages};
#[cfg(test)]
const MAX_PAGES: usize = 10;
/// Largest normalized remote body accepted for one entry.
const MAX_REMOTE_BODY: usize = 1024 * 1024;

/// Fetch a server's subscriptions without holding or writing a connection.
pub(super) fn discover(
    client: &ureq::Agent,
    server: &Server,
    stopped: &AtomicBool,
) -> Result<Vec<RemoteFeedCandidate>> {
    ensure!(!stopped.load(Ordering::Relaxed), "Collection cancelled");
    let token = token(server)?;
    let feeds: Vec<RemoteFeed> =
        get_json(client, &endpoint(server, "/v1/feeds")?, &token)
            .map_err(|error| annotate(error, &server.token_env, &token))?;
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
    ensure!(!stopped.load(Ordering::Relaxed), "Collection cancelled");
    Ok(candidates)
}

/// Fetch one bounded entry page set for a mirrored feed.
pub(super) fn fetch_entries(
    client: &ureq::Agent,
    feed: &Feed,
    server: &ServerFeed,
    stopped: &AtomicBool,
) -> Download {
    let result = request_entries(client, feed, server, stopped);
    let retry_at = result.as_ref().err().map_or(0, retry_deadline);
    Download {
        feed: feed.clone(),
        result,
        retry_at,
    }
}

fn request_entries(
    client: &ureq::Agent,
    feed: &Feed,
    server: &ServerFeed,
    stopped: &AtomicBool,
) -> Result<Option<ParsedFeed>> {
    let token = credential(server.token.as_ref(), &server.token_env)?;
    let entries = fetch_pages(client, server, &token, stopped)?;
    ensure!(!stopped.load(Ordering::Relaxed), "Collection cancelled");
    let sync_after_id = entries.after_id;
    let (articles, states, latest) =
        normalize_entries(server, entries.entries)?;
    let initialized = server.initialized || entries.complete;
    let last_changed = if sync_after_id > 0 || !initialized {
        server.last_changed
    } else {
        latest
    };
    let title = content::label(&feed.title, 200);
    Ok(Some(ParsedFeed {
        title,
        articles,
        remote: Some(RemoteFeedUpdate {
            server_id: server.server_id,
            remote_id: server.remote_id,
            initialized,
            history: entries.history,
            last_changed,
            sync_after_id,
            sync_latest: latest,
            states,
        }),
    }))
}

fn fetch_pages(
    client: &ureq::Agent,
    server: &ServerFeed,
    token: &str,
    stopped: &AtomicBool,
) -> Result<Collected> {
    collect_pages(server, stopped, |cursor, limit| {
        let url = entry_page_url(server, !server.initialized, cursor, limit)?;
        get_json(client, &url, token)
            .map_err(|error| annotate(error, &server.token_env, token))
    })
}

fn entry_page_url(
    server: &ServerFeed,
    initial: bool,
    cursor: i64,
    limit: usize,
) -> Result<url::Url> {
    let mut url = endpoint_parts(
        &server.server_url,
        &format!("/v1/feeds/{}/entries", server.remote_id),
    )?;
    {
        let mut query = url.query_pairs_mut();
        query.append_pair("limit", &limit.to_string());
        if initial {
            query
                .append_pair("order", "id")
                .append_pair("direction", "desc");
            if cursor > 0 {
                query.append_pair("before_entry_id", &cursor.to_string());
            }
        } else {
            query
                .append_pair("after_entry_id", &cursor.to_string())
                .append_pair("order", "id")
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
    let mut last_changed = server.last_changed.max(server.sync_latest);
    for entry in entries.into_values() {
        ensure!(
            entry.feed_id == server.remote_id,
            "Remote entry belongs to another feed"
        );
        let changed = chrono::DateTime::parse_from_rfc3339(&entry.changed_at)
            .context("Invalid remote entry change timestamp")?
            .timestamp();
        last_changed = last_changed.max(changed);
        if !matches!(entry.status.as_str(), "read" | "unread") {
            continue;
        }
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

fn token(server: &Server) -> Result<std::borrow::Cow<'_, str>> {
    credential(server.token.as_ref(), &server.token_env)
}

fn credential<'a>(
    inline: Option<&'a crate::model::ApiToken>,
    environment: &str,
) -> Result<std::borrow::Cow<'a, str>> {
    if let Some(token) = inline {
        ensure!(environment.is_empty(), "Ambiguous server credentials");
        token.validate()?;
        Ok(std::borrow::Cow::Borrowed(token.expose()))
    } else {
        token_from(environment).map(std::borrow::Cow::Owned)
    }
}

fn token_from(name: &str) -> Result<String> {
    let value = std::env::var(name).map_err(|_| {
        anyhow::anyhow!("Set {name} to a valid Miniflux API token")
    })?;
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
        .max_redirects(0)
        .http_status_as_error(false)
        .build()
        .call()?;
    if response.status() != 200 {
        return Err(HttpFailure {
            status: response.status().as_u16(),
            retry_at: response
                .headers()
                .get("retry-after")
                .and_then(|value| value.to_str().ok())
                .and_then(|value| super::download::retry_after(value, now()))
                .unwrap_or(0),
        }
        .into());
    }
    let bytes = crate::http_body::read(response.body_mut(), MAX_FEED_BYTES)
        .context("Miniflux response exceeds 16 MiB")?;
    serde_json::from_slice(&bytes).context("Invalid Miniflux JSON response")
}

fn annotate(
    error: anyhow::Error,
    token_env: &str,
    token: &str,
) -> anyhow::Error {
    if let Some(failure) = error.downcast_ref::<HttpFailure>() {
        if matches!(failure.status, 401 | 403) {
            let location = if token_env.is_empty() {
                "server token in TOML"
            } else {
                token_env
            };
            return anyhow::anyhow!(
                "Miniflux authentication failed; check {location}"
            );
        }
        return error;
    }
    anyhow::anyhow!(format!("{error:#}").replace(token, "[redacted]"))
}

#[derive(Debug)]
struct HttpFailure {
    status: u16,
    retry_at: i64,
}

impl std::fmt::Display for HttpFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Miniflux returned HTTP {}", self.status)
    }
}

impl std::error::Error for HttpFailure {}

pub(super) fn retry_deadline(error: &anyhow::Error) -> i64 {
    error
        .downcast_ref::<HttpFailure>()
        .map_or(0, |error| error.retry_at)
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
    total: usize,
    entries: Vec<RemoteEntry>,
}

#[derive(serde::Deserialize)]
struct RemoteEntry {
    id: i64,
    feed_id: i64,
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
        let initial = entry_page_url(&initial_server, true, 0, 200)?;
        let query = parameters(&initial);
        assert_eq!(query.get("limit").map(String::as_str), Some("200"));
        assert!(!query.contains_key("offset"));
        assert_eq!(query.get("order").map(String::as_str), Some("id"));
        assert_eq!(query.get("direction").map(String::as_str), Some("desc"));
        assert!(!query.contains_key("changed_after"));

        let incremental = entry_page_url(&server(true, 100), false, 200, 200)?;
        let query = parameters(&incremental);
        assert_eq!(
            query.get("after_entry_id").map(String::as_str),
            Some("200")
        );
        assert_eq!(query.get("order").map(String::as_str), Some("id"));
        assert_eq!(query.get("direction").map(String::as_str), Some("asc"));
        assert_eq!(query.get("changed_after").map(String::as_str), Some("99"));
        Ok(())
    }

    #[test]
    fn timestamps_accept_rfc3339_and_reject_invalid_values() {
        assert_eq!(timestamp("2026-10-01T10:00:00Z"), 1_790_848_800);
        assert_eq!(timestamp("not-a-date"), 0);
    }

    #[test]
    fn incremental_batches_resume_timestamp_ties_instead_of_starting_over()
    -> Result<()> {
        let mut remote = server(true, 100);
        let first =
            collect_pages(&remote, &AtomicBool::new(false), |cursor, _| {
                synthetic_page(cursor)
            })?;
        assert_eq!(first.entries.len(), 2000);
        assert_eq!(first.after_id, 2000);
        remote.sync_after_id = first.after_id;
        let second =
            collect_pages(&remote, &AtomicBool::new(false), |cursor, _| {
                synthetic_page(cursor)
            })?;
        assert_eq!(second.entries.len(), 1);
        assert_eq!(second.after_id, 0);
        assert!(second.entries.contains_key(&2001));
        Ok(())
    }

    #[test]
    fn initial_import_keeps_only_the_newest_bounded_set() -> Result<()> {
        let mut calls = 0;
        let collected = collect_pages(
            &server(false, 0),
            &AtomicBool::new(false),
            |before, _| {
                calls += 1;
                let offset = if before == 0 { 0 } else { 5001 - before };
                Ok(EntryPage {
                    total: 5000,
                    entries: (5000 - offset - 199..=5000 - offset)
                        .rev()
                        .map(entry)
                        .collect(),
                })
            },
        )?;
        assert_eq!(calls, MAX_PAGES);
        assert_eq!(collected.entries.len(), 2000);
        assert_eq!(collected.after_id, 0);
        assert_eq!(
            collected.entries.first_key_value().map(|(id, _)| *id),
            Some(3001)
        );
        assert_eq!(
            collected.entries.last_key_value().map(|(id, _)| *id),
            Some(5000)
        );
        Ok(())
    }

    #[test]
    fn oversized_pages_and_invalid_remote_identities_fail() -> Result<()> {
        let remote = server(true, 100);
        let bad = collect_pages(&remote, &AtomicBool::new(false), |_, _| {
            let mut page = synthetic_page(0)?;
            page.entries.push(entry(201));
            Ok(page)
        });
        assert!(bad.is_err());
        let bad = collect_pages(&remote, &AtomicBool::new(false), |_, _| {
            Ok(EntryPage {
                total: 1,
                entries: vec![entry(0)],
            })
        });
        assert!(bad.is_err());
        let mut wrong_feed = entry(1);
        wrong_feed.feed_id = 9;
        assert!(
            normalize_entries(&remote, BTreeMap::from([(1, wrong_feed)]))
                .is_err()
        );
        Ok(())
    }

    #[test]
    fn completed_id_ordered_passes_retain_the_highest_earlier_timestamp()
    -> Result<()> {
        let mut remote = server(true, 100);
        remote.sync_latest = 2_000_000_000;
        let (_, _, latest) =
            normalize_entries(&remote, BTreeMap::from([(1, entry(1))]))?;
        assert_eq!(latest, 2_000_000_000);
        let mut invalid = entry(2);
        invalid.changed_at = "not-a-date".into();
        assert!(
            normalize_entries(&remote, BTreeMap::from([(2, invalid)])).is_err()
        );
        Ok(())
    }

    #[test]
    fn errors_redact_the_exact_api_token() {
        let error = annotate(
            anyhow::anyhow!("Invalid value: secret-token"),
            "TOKEN",
            "secret-token",
        );
        assert!(!error.to_string().contains("secret-token"));
        assert!(error.to_string().contains("[redacted]"));
    }

    #[test]
    fn pagination_survives_deletions_before_the_last_remote_id() -> Result<()> {
        let remote = server(true, 100);
        let mut calls = 0;
        let collected =
            collect_pages(&remote, &AtomicBool::new(false), |cursor, _| {
                calls += 1;
                if cursor == 0 {
                    synthetic_page(0)
                } else {
                    assert_eq!(cursor, 200);
                    Ok(EntryPage {
                        total: 1,
                        entries: vec![entry(201)],
                    })
                }
            })?;
        assert_eq!(calls, 2);
        assert!(collected.entries.contains_key(&201));
        assert_eq!(collected.after_id, 0);
        Ok(())
    }

    #[test]
    fn cancelled_or_repeating_pagination_stops_before_another_request()
    -> Result<()> {
        let remote = server(true, 100);
        let stopped = AtomicBool::new(false);
        let mut calls = 0;
        let result = collect_pages(&remote, &stopped, |_, _| {
            calls += 1;
            stopped.store(true, Ordering::Relaxed);
            synthetic_page(0)
        });
        assert!(result.is_err());
        assert_eq!(calls, 1);
        let mut calls = 0;
        let result = collect_pages(&remote, &AtomicBool::new(false), |_, _| {
            calls += 1;
            synthetic_page(0)
        });
        assert!(result.is_err());
        assert_eq!(calls, 2);
        Ok(())
    }

    #[test]
    fn redaction_keeps_http_retry_deadlines() {
        let error = annotate(
            HttpFailure {
                status: 429,
                retry_at: 1234,
            }
            .into(),
            "TOKEN",
            "secret",
        );
        assert_eq!(retry_deadline(&error), 1234);
        assert!(error.to_string().contains("HTTP 429"));
    }

    #[test]
    fn initial_history_resumes_by_identity_and_obeys_a_partial_last_page()
    -> Result<()> {
        let mut remote = server(false, 0);
        remote.sync.initial_entries = 5;
        remote.sync.entries_per_page = 2;
        remote.sync.pages_per_sync = 1;
        for (before, expected, ids) in
            [(0, 2, vec![10, 9]), (9, 2, vec![8, 6]), (6, 1, vec![5])]
        {
            let collected = collect_pages(
                &remote,
                &AtomicBool::new(false),
                |cursor, limit| {
                    assert_eq!((cursor, limit), (before, expected));
                    Ok(EntryPage {
                        total: 10,
                        entries: ids.iter().copied().map(entry).collect(),
                    })
                },
            )?;
            remote.history = collected.history;
            assert_eq!(collected.complete, remote.history.loaded == 5);
            assert_eq!(collected.after_id, 0);
        }
        assert_eq!(remote.history.loaded, 5);
        assert_eq!(remote.history.before_id, 5);
        let query = parameters(&entry_page_url(&remote, true, 6, 1)?);
        assert_eq!(query.get("before_entry_id").map(String::as_str), Some("6"));
        assert!(!query.contains_key("offset"));
        Ok(())
    }

    #[test]
    fn unlimited_history_completes_only_when_the_server_has_no_more_entries()
    -> Result<()> {
        let mut remote = server(false, 0);
        remote.sync.initial_entries = 0;
        remote.sync.entries_per_page = 2;
        remote.sync.pages_per_sync = 1;
        let first = collect_pages(&remote, &AtomicBool::new(false), |_, _| {
            Ok(EntryPage {
                total: 3,
                entries: vec![entry(4), entry(3)],
            })
        })?;
        assert!(!first.complete);
        remote.history = first.history;
        let last =
            collect_pages(&remote, &AtomicBool::new(false), |cursor, _| {
                assert_eq!(cursor, 3);
                Ok(EntryPage {
                    total: 1,
                    entries: vec![entry(1)],
                })
            })?;
        assert!(last.complete);
        assert_eq!(last.history.loaded, 3);
        assert_eq!(last.history.total, 3);
        Ok(())
    }

    fn synthetic_page(offset: i64) -> Result<EntryPage> {
        Ok(EntryPage {
            total: (2001 - offset) as usize,
            entries: (offset..(offset + 200).min(2001))
                .map(|index| entry(index + 1))
                .collect(),
        })
    }

    fn entry(id: i64) -> RemoteEntry {
        RemoteEntry {
            id,
            feed_id: 7,
            title: format!("Entry {id}"),
            url: format!("https://publisher.example/{id}"),
            author: String::new(),
            content: "<p>Cached content.</p>".into(),
            published_at: "2026-10-01T10:00:00Z".into(),
            changed_at: "2026-10-01T10:01:00Z".into(),
            status: "unread".into(),
            starred: false,
        }
    }

    fn server(initialized: bool, last_changed: i64) -> ServerFeed {
        ServerFeed {
            server_id: 1,
            server_url: "https://reader.example".into(),
            token_env: "LUME_MINIFLUX_TOKEN".into(),
            token: None,
            enabled: true,
            remote_id: 7,
            initialized,
            last_changed,
            sync_after_id: 0,
            sync_latest: last_changed,
            present: true,
            sync: Default::default(),
            history: Default::default(),
        }
    }

    fn parameters(url: &url::Url) -> BTreeMap<String, String> {
        url.query_pairs()
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect()
    }
}
