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
fn collector_accepts_the_inclusive_decoded_limit_with_or_without_gzip()
-> Result<()> {
    // Owned RSS plus trailing XML whitespace. The gzip fixture decodes to
    // exactly 16 MiB; its compressed size is only about 17 KiB.
    let mut plain = include_bytes!("fixtures/rss.xml").to_vec();
    plain.resize(source::MAX_FEED_BYTES, b' ');
    let gzip = include_bytes!("fixtures/feed-at-limit.xml.gz");
    for (body, encoding) in [
        (plain.as_slice(), ""),
        (gzip.as_slice(), "Content-Encoding: gzip\r\n"),
    ] {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let store = Store::open(&path)?;
        let headers = format!("{encoding}ETag: \"accepted\"\r\n");
        let (address, request) = server(200, body, &headers)?;
        store.subscribe(&address)?;

        assert_eq!(source::collect_once(&path)?, (1, 0));

        request.recv_timeout(Duration::from_secs(5))?;
        assert_eq!(store.feeds()?[0].etag, "\"accepted\"");
        assert!(store.feeds()?[0].error.is_empty());
        let article = store.library()?.articles.remove(0);
        assert_eq!(article.title, "Cache & consequence");
        assert!(
            store
                .body(article.id)?
                .contains("Keep **one** copy & measure.")
        );
    }
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn decoded_overflow_preserves_cached_content_and_validators() -> Result<()> {
    // The other owned gzip fixture adds exactly one whitespace byte.
    let mut plain = include_bytes!("fixtures/rss.xml").to_vec();
    plain.resize(source::MAX_FEED_BYTES + 1, b' ');
    let gzip = include_bytes!("fixtures/feed-over-limit.xml.gz");
    for (body, encoding) in [
        (plain.as_slice(), ""),
        (gzip.as_slice(), "Content-Encoding: gzip\r\n"),
    ] {
        let (directory, store, _feed) = common::seeded_store()?;
        let path = directory.path().join("library.sqlite3");
        let article = store.library()?.articles.remove(0);
        let before = store.body(article.id)?;
        let (address, _request) =
            server(200, body, &format!("{encoding}ETag: \"rejected\"\r\n"))?;
        replace_address(&path, &address)?;

        assert_eq!(source::collect_once(&path)?, (1, 1));

        let feed = store.feeds()?.remove(0);
        assert_eq!(feed.error, "Feed exceeds 16 MiB");
        assert!(feed.etag.is_empty());
        assert_eq!(store.body(article.id)?, before);
        assert_eq!(store.source_health()?[0].failures, 1);
    }
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn discovery_accepts_a_full_history_feed_larger_than_four_mib() -> Result<()> {
    let mut body = include_bytes!("fixtures/rss.xml").to_vec();
    body.resize(5 * 1024 * 1024, b' ');
    let (address, request) = server(200, &body, "")?;

    let candidates = source::discovery::discover(&address)?;

    request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].title, "Systems Journal");
    assert_eq!(candidates[0].url, address);
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
    server_after_delay(status, body, headers, Duration::ZERO)
}

fn server_after_delay(
    status: u16,
    body: &[u8],
    headers: &str,
    delay: Duration,
) -> Result<(String, mpsc::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0").context(
        "Real HTTP qualification requires loopback socket permission",
    )?;
    let address = format!("http://{}/feed", listener.local_addr()?);
    let body = body.to_vec();
    let headers = headers.to_owned();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        thread::sleep(delay);
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

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn decoded_article_boundaries_cover_gzip_and_preserve_a_good_cache()
-> Result<()> {
    let exact = include_bytes!("fixtures/http/article-exact.gz");
    let over = include_bytes!("fixtures/http/article-over.gz");
    for (body, accepted) in [
        (exact.as_slice(), true),
        (over.as_slice(), false),
        (b"broken gzip".as_slice(), false),
    ] {
        let (directory, mut store, feed) = common::seeded_store()?;
        let (address, request) = server(
            200,
            body,
            "Content-Type: text/html\r\nContent-Encoding: gzip\r\n",
        )?;
        let target = set_article_url(&mut store, &feed, &address)?;
        store.confirm_article_rule(&RulePreview {
            target: target.clone(),
            feed: feed.id,
            previous: String::new(),
            selector: "#article".into(),
            document: ExtractedDocument {
                final_url: address,
                body: "Previous successful article".into(),
            },
        })?;
        let result = source::article::fetch_once(
            &directory.path().join("library.sqlite3"),
            target.article,
        );
        assert_eq!(result.is_ok(), accepted, "{result:?}");
        let cache = store.cached_document(target.article)?.context("Cache")?;
        if accepted {
            assert!(cache.body.contains("Durable writes"));
        } else {
            assert_eq!(cache.body, "Previous successful article");
            assert_eq!(cache.revision, 1);
        }
        request.recv_timeout(Duration::from_secs(5))?;
    }
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn offline_package_downloads_full_text_and_pins_validated_svg() -> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg" width="80" height="40"><rect width="80" height="40" fill="orange"/></svg>"#;
    let (image, image_request) =
        server(200, svg, "Content-Type: image/svg+xml\r\n")?;
    let html = format!(
        "<html><body><article><h1>Offline qualification</h1><p>Readable independent article content with durable offline availability and a useful diagram.</p><img src='{image}' alt='Diagram'/></article></body></html>"
    );
    let (address, article_request) =
        server(200, html.as_bytes(), "Content-Type: text/html\r\n")?;
    let target = set_article_url(&mut store, &feed, &address)?;
    store.set_queued(target.article, true)?;
    let plan = lume::offline::capture(
        &store,
        lume::model::View::Queue,
        lume::model::OfflineLimits::default(),
    )?;
    let images = directory.path().join("images");
    corrupt_image_caches(&images, &image)?;

    let result = lume::offline::prepare(
        &store,
        &images,
        plan,
        &std::sync::atomic::AtomicBool::new(false),
        |_| {},
    )?;

    article_request.recv_timeout(Duration::from_secs(5))?;
    image_request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(
        result.items[0].status,
        lume::model::OfflineStatus::Ready,
        "{result:?}"
    );
    assert_eq!(result.items[0].images, 1);
    assert_eq!(
        lume::media::cache::read(&images, &image)?.context("Pinned image")?,
        svg
    );
    assert!(lume::media::cache::path(&images.join("offline"), &image).exists());
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn offline_failed_image_keeps_readable_text_and_reports_partial() -> Result<()>
{
    let (directory, mut store, feed) = common::seeded_store()?;
    let (image, image_request) =
        server(503, b"Unavailable", "Content-Type: text/plain\r\n")?;
    let html = format!(
        "<html><body><article><h1>Partial qualification</h1><p>Useful full article remains readable even when its image server cannot supply a diagram.</p><img src='{image}' alt='Diagram'/></article></body></html>"
    );
    let (address, article_request) =
        server(200, html.as_bytes(), "Content-Type: text/html\r\n")?;
    let target = set_article_url(&mut store, &feed, &address)?;
    store.set_queued(target.article, true)?;
    let plan = lume::offline::capture(
        &store,
        lume::model::View::Queue,
        lume::model::OfflineLimits::default(),
    )?;

    let result = lume::offline::prepare(
        &store,
        &directory.path().join("images"),
        plan,
        &std::sync::atomic::AtomicBool::new(false),
        |_| {},
    )?;

    article_request.recv_timeout(Duration::from_secs(5))?;
    image_request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(result.items[0].status, lume::model::OfflineStatus::Partial);
    assert!(result.items[0].error.contains("503"));
    assert!(
        store
            .cached_document(target.article)?
            .context("Full cache")?
            .body
            .contains("Useful full article")
    );
    assert!(
        lume::media::cache::read(&directory.path().join("images"), &image)?
            .is_none()
    );
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn linked_http_reading_stays_temporary_until_explicitly_saved() -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_directory, store, _) = common::seeded_store()?;
    let (address, request) = server(
        200,
        include_bytes!("fixtures/article.html"),
        "Content-Type: text/html\r\n",
    )?;
    let mut app = lume::app::App::new(store, false)?;
    let original = app.current().context("Original article")?.id;
    app.set_network_enabled(true);
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(&format!("open-page {address}"))?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.web_pending() {
        app.poll_web()?;
        ensure!(
            std::time::Instant::now() < deadline,
            "Linked reading timed out"
        );
        thread::sleep(Duration::from_millis(1));
    }
    request.recv_timeout(Duration::from_secs(5))?;
    assert!(
        app.web_page()
            .context("Page")?
            .rendered
            .text
            .contains("Durable")
    );
    assert_eq!(app.current().context("Original article")?.id, original);
    assert_eq!(app.library.articles.len(), 1);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(!app.web_active());
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn resizing_during_linked_http_never_replaces_the_new_navigation_with_old_layout()
-> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let (_directory, store, _) = common::seeded_store()?;
    store.save_web_document(&lume::model::WebDocument {
        url: "https://example.org/previous".into(),
        title: "Previous page".into(),
        body: "# Previous page\n\nThis previous page must not win.".into(),
    })?;
    let mut app = lume::app::App::new(store, false)?;
    app.set_network_enabled(true);
    open_linked(&mut app, "https://example.org/previous")?;
    settle_linked(&mut app)?;
    let (address, request) = server_after_delay(
        200,
        include_bytes!("fixtures/article.html"),
        "Content-Type: text/html\r\n",
        Duration::from_millis(200),
    )?;

    open_linked(&mut app, &address)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 50, 30));
    app.poll_web()?;
    settle_linked(&mut app)?;

    request.recv_timeout(Duration::from_secs(5))?;
    assert_eq!(app.web_page().context("New page")?.document.url, address);
    assert!(
        app.web_page()
            .context("New page")?
            .rendered
            .text
            .contains("Durable")
    );
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(
        app.web_page().context("Previous page")?.document.url,
        "https://example.org/previous"
    );
    Ok(())
}

fn open_linked(app: &mut lume::app::App, address: &str) -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(&format!("open-page {address}"))?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn settle_linked(app: &mut lume::app::App) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.web_pending() {
        app.poll_web()?;
        ensure!(
            std::time::Instant::now() < deadline,
            "Linked reading timed out"
        );
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn corrupt_image_caches(
    directory: &std::path::Path,
    address: &str,
) -> Result<()> {
    for path in [directory.to_path_buf(), directory.join("offline")] {
        lume::media::cache::write(&path, address, b"corrupt image cache")?;
    }
    Ok(())
}
