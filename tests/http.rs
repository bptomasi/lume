//! Real HTTP qualification. Run explicitly with `make check-release`.
//!
//! These tests require loopback sockets and never silently bypass a failed
//! bind. Ordinary local checks do not start a server.

mod common;

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc,
    thread,
    time::Duration,
};

use anyhow::{Context, Result, ensure};
use lume::{
    model::{ArticleTarget, ExtractedDocument, RulePreview},
    source,
    store::Store,
};

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn collector_retry_after_is_durable_and_does_not_immediately_hit_the_server_again()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let (address, request) =
        server(429, b"Retry later", "Retry-After: 120\r\n")?;
    for feed in store.feeds()? {
        store.unsubscribe(feed.id)?;
    }
    let id = store.subscribe(&address)?;

    assert_eq!(
        source::collect_once(&directory.path().join("library.sqlite3"))?,
        (1, 1)
    );
    request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(
        source::collect_once(&directory.path().join("library.sqlite3"))?,
        (0, 0)
    );

    let health = store.source_health()?.remove(0);
    assert_eq!((health.feed, health.failures), (id, 1));
    assert!(health.retry_at >= health.attempted_at + 120);
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn article_worker_applies_the_reviewed_source_selector_on_real_http()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let page = br#"<html><body><main id="picked"><h1>Selected content</h1><p>The explicit source rule owns this chosen paragraph about durable writes.</p></main><aside>Unrelated navigation marker</aside></body></html>"#;
    let (address, request) = server(200, page, "Content-Type: text/html\r\n")?;
    let target = set_article_url(&mut store, &feed, &address)?;
    store.confirm_article_rule(&RulePreview {
        target: target.clone(),
        feed: feed.id,
        previous: String::new(),
        selector: "#picked".into(),
        document: ExtractedDocument {
            final_url: address,
            body: "Previously reviewed setup content.".into(),
        },
    })?;

    source::article::fetch_once(
        &directory.path().join("library.sqlite3"),
        target.article,
    )?;
    request.recv_timeout(Duration::from_secs(5))?;

    let cache = store
        .cached_document(target.article)?
        .context("Missing result")?;
    assert!(cache.body.contains("Selected content"));
    assert!(!cache.body.contains("Unrelated navigation marker"));
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn full_article_redirect_uses_final_url_and_preserves_feed_content()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let before = store.body(store.library()?.articles[0].id)?;
    let (destination, destination_request) = server(
        200,
        include_bytes!("fixtures/article.html"),
        "Content-Type: text/html; charset=utf-8\r\n",
    )?;
    let (origin, origin_request) =
        server(302, b"", &format!("Location: {destination}\r\n"))?;
    let target = set_article_url(&mut store, &feed, &origin)?;

    source::article::fetch_once(
        &directory.path().join("library.sqlite3"),
        target.article,
    )?;

    origin_request.recv_timeout(Duration::from_secs(5))?;
    let request = destination_request.recv_timeout(Duration::from_secs(5))?;
    assert!(request.to_lowercase().contains("accept: text/html"));
    let cache = store
        .cached_document(target.article)?
        .context("Cache missing")?;
    assert_eq!(cache.url, origin);
    assert_eq!(cache.final_url, destination);
    assert!(cache.body.contains("~~~rust"));
    assert!(
        cache
            .body
            .contains(&destination.replace("/feed", "/reference#recovery"))
    );
    assert_eq!(store.body(target.article)?, before);
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn non_html_and_http_errors_keep_the_last_successful_full_article() -> Result<()>
{
    let (directory, mut store, feed) = common::seeded_store()?;
    for (status, headers) in
        [(200, "Content-Type: application/pdf\r\n"), (403, "")]
    {
        let (address, request) =
            server(status, b"Unavailable article", headers)?;
        let target = set_article_url(&mut store, &feed, &address)?;
        store.cache_document(
            &target,
            &ExtractedDocument {
                final_url: address,
                body: "Previously cached usable text.".into(),
            },
        )?;

        let result = source::article::fetch_once(
            &directory.path().join("library.sqlite3"),
            target.article,
        );

        assert!(result.is_err());
        request.recv_timeout(Duration::from_secs(5))?;
        let cache = store
            .cached_document(target.article)?
            .context("Cache missing")?;
        assert_eq!(cache.body, "Previously cached usable text.");
        assert!(!cache.error.is_empty());
    }
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn oversized_download_is_rejected_before_caching() -> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let (address, _request) = server(
        200,
        &vec![b'x'; 2 * 1024 * 1024 + 1],
        "Content-Type: text/html\r\n",
    )?;
    let target = set_article_url(&mut store, &feed, &address)?;

    let result = source::article::fetch_once(
        &directory.path().join("library.sqlite3"),
        target.article,
    );

    assert!(result.is_err());
    let cache = store
        .cached_document(target.article)?
        .context("Failure missing")?;
    assert!(cache.body.is_empty());
    assert_eq!(cache.revision, 0);
    assert!(!cache.error.is_empty());
    Ok(())
}

fn set_article_url(
    store: &mut Store,
    feed: &lume::model::Feed,
    address: &str,
) -> Result<ArticleTarget> {
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?.articles;
    incoming[0].url = address.into();
    store.ingest(feed, "Systems Journal", &incoming)?;
    Ok(ArticleTarget {
        article: store.library()?.articles[0].id,
        url: address.into(),
    })
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn conditional_http_collection_commits_content_and_handles_304() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("http.sqlite3");
    let store = Store::open(&path)?;
    let (address, first) = server(
        200,
        include_bytes!("fixtures/rss.xml"),
        "ETag: \"version-1\"\r\n",
    )?;
    store.subscribe(&address)?;
    assert_eq!(source::collect_once(&path)?, (1, 0));
    let request = first.recv_timeout(Duration::from_secs(5))?;
    assert!(request.starts_with("GET /feed HTTP/1.1\r\n"));
    assert!(!request.to_lowercase().contains("if-none-match"));
    assert_eq!(store.library()?.articles.len(), 1);
    assert_eq!(store.feeds()?[0].etag, "\"version-1\"");

    let (replacement, second) = server(304, b"", "")?;
    replace_address(&path, &replacement)?;
    assert_eq!(source::collect_once(&path)?, (1, 0));
    let request = second.recv_timeout(Duration::from_secs(5))?;
    assert!(
        request
            .to_lowercase()
            .contains("if-none-match: \"version-1\"")
    );
    assert_eq!(store.library()?.articles.len(), 1);
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn http_failure_preserves_cached_articles_and_records_the_error() -> Result<()>
{
    let (directory, store, _feed) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let (address, request) = server(503, b"unavailable", "")?;
    replace_address(&path, &address)?;
    assert_eq!(source::collect_once(&path)?, (1, 1));
    request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(store.library()?.articles.len(), 1);
    assert!(!store.feeds()?[0].error.is_empty());
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn discovery_follows_redirect_and_returns_advertised_choices() -> Result<()> {
    let html = br#"<html><head>
        <link rel="alternate" type="application/rss+xml" href="/rss">
        </head></html>"#;
    let (destination, destination_request) = server(200, html, "")?;
    let headers = format!("Location: {destination}\r\n");
    let (origin, origin_request) = server(302, b"", &headers)?;

    let choices = source::discovery::discover(&origin)?;

    origin_request.recv_timeout(Duration::from_secs(5))?;
    destination_request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].url, destination.replace("/feed", "/rss"));
    Ok(())
}

fn replace_address(path: &std::path::Path, address: &str) -> Result<()> {
    let connection = rusqlite::Connection::open(path)?;
    connection.execute("UPDATE feeds SET url = ?1", [address])?;
    Ok(())
}

fn server(
    status: u16,
    body: &[u8],
    headers: &str,
) -> Result<(String, mpsc::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0").context(
        "Real HTTP qualification requires loopback socket permission",
    )?;
    let address = format!("http://{}/feed", listener.local_addr()?);
    let body = body.to_vec();
    let headers = headers.to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        if let Ok(request) = serve(listener, status, &body, &headers) {
            let _ = sender.send(request);
        }
    });
    Ok((address, receiver))
}

fn serve(
    listener: TcpListener,
    status: u16,
    body: &[u8],
    headers: &str,
) -> Result<String> {
    listener.set_nonblocking(true)?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    let mut stream = loop {
        match listener.accept() {
            Ok((stream, _)) => break stream,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                ensure!(
                    std::time::Instant::now() < deadline,
                    "No HTTP client connected"
                );
                thread::sleep(Duration::from_millis(5));
            }
            Err(error) => return Err(error.into()),
        }
    };
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    let request = read_request(&mut stream)?;
    // CRLF framing is byte-sensitive HTTP wire data, not display formatting.
    let response = format!(
        "HTTP/1.1 {status} Result\r\nContent-Length: {}\r\nConnection: close\r\n{headers}\r\n",
        body.len(),
    );
    stream.write_all(response.as_bytes())?;
    stream.write_all(body)?;
    Ok(request)
}

fn read_request(stream: &mut std::net::TcpStream) -> Result<String> {
    let mut request = Vec::new();
    while !request.ends_with(b"\r\n\r\n") {
        let mut byte = [0];
        ensure!(stream.read(&mut byte)? == 1, "Truncated HTTP request");
        request.push(byte[0]);
        ensure!(request.len() <= 8192, "Oversized HTTP request");
    }
    Ok(String::from_utf8(request)?)
}
