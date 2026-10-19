//! Required real HTTP image boundaries; ordinary checks never open sockets.

mod common;

use std::{
    io::{Read, Write},
    net::TcpListener,
    sync::mpsc::{self, Receiver, SyncSender},
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    media, source,
    store::Store,
};

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="40"><rect width="100" height="40" fill="red"/></svg>"#;

struct Server {
    url: String,
    request: Receiver<String>,
    release: SyncSender<()>,
    finished: Receiver<Result<(), String>>,
}

fn server(status: u16, headers: &str, body: &[u8]) -> Result<Server> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let url = format!("http://{}/diagram", listener.local_addr()?);
    let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\n{headers}Connection: close\r\n\r\n", body.len()).into_bytes();
    let body = body.to_vec();
    let (send_request, request) = mpsc::sync_channel(1);
    let (release, released) = mpsc::sync_channel(1);
    let (done, finished) = mpsc::sync_channel(1);
    thread::spawn(move || {
        let result = (|| -> Result<()> {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            stream.set_write_timeout(Some(Duration::from_secs(5)))?;
            let mut bytes = [0; 8192];
            let length = stream.read(&mut bytes)?;
            send_request
                .send(String::from_utf8_lossy(&bytes[..length]).into_owned())?;
            released.recv_timeout(Duration::from_secs(5))?;
            stream.write_all(&response)?;
            stream.write_all(&body)?;
            Ok(())
        })()
        .map_err(|error| error.to_string());
        let _ = done.send(result);
    });
    Ok(Server {
        url,
        request,
        release,
        finished,
    })
}

fn reading(url: &str) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body =
        format!("Before image.\n\n![Diagram]({url})\n\nAfter image.");
    store.ingest(&feed, &incoming.title, &incoming.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(70, 20);
    app.enable_images(directory.path().join("images"), false)?;
    Ok((directory, app))
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        app.poll_images();
        if !app.images_pending() {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Image worker timed out");
        thread::sleep(Duration::from_millis(1));
    }
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn http_image_download_is_cached_and_reopens_in_an_offline_session()
-> Result<()> {
    let server = server(200, "Content-Type: image/svg+xml\r\n", SVG)?;
    let (directory, mut app) = reading(&server.url)?;
    let key = app.image_key(&app.images.slots[0]).context("Image key")?;
    app.poll_images();
    let request = server.request.recv_timeout(Duration::from_secs(5))?;
    assert!(request.contains("image/svg+xml"));
    assert!(!request.to_lowercase().contains("cookie:"));
    server.release.send(())?;
    settle(&mut app)?;
    server
        .finished
        .recv_timeout(Duration::from_secs(5))?
        .map_err(anyhow::Error::msg)?;
    assert!(app.images.thumbnail(&key).is_some());

    let mut offline = App::new(
        Store::open(&directory.path().join("library.sqlite3"))?,
        false,
    )?;
    offline.focus = Focus::Reader;
    offline.resize_reader(70, 20);
    offline.enable_images(directory.path().join("images"), true)?;
    settle(&mut offline)?;

    assert!(offline.images.thumbnail(&key).is_some());
    assert!(
        media::cache::path(&directory.path().join("images"), &key.url)
            .is_file()
    );
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn delayed_http_images_leave_cursor_input_available_and_do_not_bleed_after_navigation()
-> Result<()> {
    let server = server(200, "Content-Type: image/svg+xml\r\n", SVG)?;
    let (directory, mut app) = reading(&server.url)?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = store.feeds()?.remove(0);
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].external_id = "second-article".into();
    incoming.articles[0].published -= 1;
    incoming.articles[0].body = "Independent article without images.".into();
    store.ingest(&feed, &incoming.title, &incoming.articles)?;
    app.reload()?;
    app.poll_images();
    server.request.recv_timeout(Duration::from_secs(5))?;

    let started = Instant::now();
    app.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))?;
    assert!(started.elapsed() < Duration::from_secs(1));
    assert_eq!(app.reader_column, 1);
    assert!(app.images_pending());
    app.focus = Focus::Articles;
    app.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))?;
    let current = app.current().context("Next article")?.id;
    server.release.send(())?;
    settle(&mut app)?;

    assert_eq!(app.current().map(|article| article.id), Some(current));
    assert_eq!(app.body, "Independent article without images.");
    assert!(app.images.slots.is_empty());
    server
        .finished
        .recv_timeout(Duration::from_secs(5))?
        .map_err(anyhow::Error::msg)?;
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn delayed_inspection_discards_old_crops_without_blocking_zoom_or_close()
-> Result<()> {
    let server = server(200, "Content-Type: image/svg+xml\r\n", SVG)?;
    let (_directory, mut app) = reading(&server.url)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 144, 44));
    app.reader_cursor = app.images.slots[0].caption;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    let old = app.image_inspector_key().context("Initial inspection")?;
    app.poll_images();
    server.request.recv_timeout(Duration::from_secs(5))?;

    let started = Instant::now();
    app.key(KeyEvent::new(KeyCode::Char('+'), KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))?;
    let newest = app.image_inspector_key().context("New crop")?;
    assert_ne!(old, newest);
    assert!(started.elapsed() < Duration::from_secs(1));
    server.release.send(())?;
    settle(&mut app)?;

    assert!(app.images.thumbnail(&newest).is_some());
    assert_eq!(app.image_inspector_key(), Some(newest));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.image_inspector_key().is_none());
    server
        .finished
        .recv_timeout(Duration::from_secs(5))?
        .map_err(anyhow::Error::msg)?;
    Ok(())
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn http_error_and_non_image_content_keep_the_article_and_show_recoverable_errors()
-> Result<()> {
    for (status, body) in [
        (404, b"Missing".as_slice()),
        (200, b"<html>Login required</html>".as_slice()),
    ] {
        let server = server(status, "Content-Type: text/html\r\n", body)?;
        let (_directory, mut app) = reading(&server.url)?;
        let key = app.image_key(&app.images.slots[0]).context("Image key")?;
        let original = app.body.clone();
        app.poll_images();
        server.request.recv_timeout(Duration::from_secs(5))?;
        server.release.send(())?;
        settle(&mut app)?;

        assert!(app.images.error(&key).is_some());
        assert!(app.images.thumbnail(&key).is_none());
        assert_eq!(app.body, original);
        server
            .finished
            .recv_timeout(Duration::from_secs(5))?
            .map_err(anyhow::Error::msg)?;
    }
    Ok(())
}
