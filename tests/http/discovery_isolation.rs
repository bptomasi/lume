//! Actual sockets prove discovery does not block unrelated durable publication.
use super::*;

struct Endpoint {
    address: String,
    requests: mpsc::Receiver<String>,
    worker: thread::JoinHandle<Result<()>>,
}

#[test]
#[ignore = "requires real loopback HTTP; required by make check-release"]
fn slow_discovery_allows_fast_server_and_publisher_commits() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let store = Store::open(&path)?;
    let (release, gate) = mpsc::channel();
    let slow = endpoint("127.0.0.1", vec![b"[]".to_vec()], Some(gate))?;
    let feeds = br#"[{"id":7,"title":"Fast","feed_url":"https://publisher.example/feed"}]"#;
    let fast = endpoint(
        "127.0.0.2",
        vec![feeds.to_vec(), br#"{"total":0,"entries":[]}"#.to_vec()],
        None,
    )?;
    let publisher = endpoint(
        "127.0.0.3",
        vec![include_bytes!("../fixtures/rss.xml").to_vec()],
        None,
    )?;
    for endpoint in [&slow, &fast] {
        store.add_server_token(&endpoint.address, &"isolated-token".into())?;
    }
    store.subscribe(&publisher.address)?;
    let collector = thread::spawn(move || source::collect_once(&path));
    let first = slow.requests.recv_timeout(Duration::from_secs(5))?;
    assert!(first.starts_with("GET /v1/feeds "));
    let ready = wait_for_other_sources(&store);
    release.send(())?;
    ready?;
    assert_eq!(
        collector
            .join()
            .map_err(|_| anyhow::anyhow!("Collector thread"))??,
        (2, 0)
    );
    for endpoint in [slow, fast, publisher] {
        endpoint
            .worker
            .join()
            .map_err(|_| anyhow::anyhow!("HTTP fixture thread"))??;
    }
    Ok(())
}

fn wait_for_other_sources(store: &Store) -> Result<()> {
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    loop {
        if store
            .source_health()?
            .iter()
            .filter(|source| source.succeeded_at > 0)
            .count()
            == 2
        {
            return Ok(());
        }
        ensure!(
            std::time::Instant::now() < deadline,
            "Slow discovery blocked other sources"
        );
        thread::sleep(Duration::from_millis(2));
    }
}

fn endpoint(
    host: &str,
    bodies: Vec<Vec<u8>>,
    gate: Option<mpsc::Receiver<()>>,
) -> Result<Endpoint> {
    let listener = TcpListener::bind((host, 0))?;
    let address = format!("http://{}", listener.local_addr()?);
    let (sender, requests) = mpsc::channel();
    let worker = thread::spawn(move || {
        for body in bodies {
            let (mut stream, _) = listener.accept()?;
            stream.set_read_timeout(Some(Duration::from_secs(5)))?;
            sender.send(read_request(&mut stream)?)?;
            if let Some(gate) = &gate {
                gate.recv_timeout(Duration::from_secs(5))?;
            }
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )?;
            stream.write_all(&body)?;
        }
        Ok(())
    });
    Ok(Endpoint {
        address,
        requests,
        worker,
    })
}
