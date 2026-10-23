//! First history commits a bounded checkpoint and resumes with a stable identity.
use super::*;

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn configured_history_resumes_after_restart_without_changing_personal_flags()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let data = directory.path().join("data");
    let config = directory.path().join("config.toml");
    let (address, requests) = history_server()?;
    std::fs::write(
        &config,
        format!(
            "[[servers]]\nid='home'\nurl='{address}'\ntoken_env='LUME_TEST_MINIFLUX_TOKEN'\n[servers.sync]\ninitial_entries=3\nentries_per_page=2\npages_per_sync=1\n"
        ),
    )?;
    first_mirror_sync(&config, &data)?;
    let store = Store::open(&data.join("library.sqlite3"))?;
    let first = store.library()?.articles[0].id;
    store.set_starred(first, true)?;
    let remote = store
        .feeds()?
        .remove(0)
        .server
        .context("Remote checkpoint")?;
    assert!(!remote.initialized);
    assert_eq!(remote.history.loaded, 2);
    assert_eq!(remote.history.before_id, 4);
    first_mirror_sync(&config, &data)?;
    let remote = store
        .feeds()?
        .remove(0)
        .server
        .context("Resumed checkpoint")?;
    assert!(remote.initialized);
    assert_eq!(remote.history.loaded, 3);
    assert_eq!(store.library()?.articles.len(), 3);
    assert!(store.article(first)?.starred);
    let requests = (0..4)
        .map(|_| requests.recv_timeout(Duration::from_secs(5)))
        .collect::<std::result::Result<Vec<_>, _>>()?;
    assert!(requests[3].contains("before_entry_id=4"));
    assert!(requests[3].contains("limit=1"));
    Ok(())
}

fn history_server() -> Result<(String, mpsc::Receiver<String>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = format!("http://{}", listener.local_addr()?);
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || -> Result<()> {
        for index in 0..4 {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            let request = read_request(&mut stream)?;
            let body = history_body(index)?;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            )?;
            sender.send(request)?;
        }
        Ok(())
    });
    Ok((address, receiver))
}

fn history_body(index: usize) -> Result<String> {
    if index.is_multiple_of(2) {
        return Ok(r#"[{"id":7,"title":"Remote","feed_url":"https://publisher.example/feed"}]"#.into());
    }
    let entries = if index == 1 { vec![5, 4] } else { vec![2] };
    Ok(serde_json::to_string(
        &serde_json::json!({"total": if index == 1 {5} else {2}, "entries": entries.into_iter().map(|id| serde_json::json!({
        "id": id, "feed_id": 7, "title": format!("Entry {id}"), "url": format!("https://publisher.example/{id}"), "author": "", "content": "<p>Body</p>",
        "published_at": "2026-10-01T10:00:00Z", "changed_at": "2026-10-01T10:01:00Z", "status": "unread", "starred": false
    })).collect::<Vec<_>>() }),
    )?)
}
