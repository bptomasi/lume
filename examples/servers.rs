//! Export an isolated Library with local feeds and cached remote connections.

mod support;

use std::path::PathBuf;

use anyhow::Result;
use lume::{
    app::{App, Focus},
    model::{
        Feed, FeedOrigin, IncomingArticle, RemoteFeedCandidate,
        RemoteFeedUpdate, View,
    },
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

fn main() -> Result<()> {
    let output = std::env::args_os()
        .nth(1)
        .map_or_else(|| PathBuf::from("/tmp/lume-servers.json"), PathBuf::from);
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let first = seed_connection(&mut store, "https://reader.example", false)?;
    seed_connection(&mut store, "https://archive.example", true)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Library;
    app.resize_workspace(Rect::new(0, 0, 144, 48));
    app.set_view(View::Server(first))?;
    let mut terminal = Terminal::new(TestBackend::new(144, 48))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    support::export(terminal.backend().buffer(), &output)?;
    println!("{}", output.display());
    Ok(())
}

fn seed_connection(store: &mut Store, root: &str, paused: bool) -> Result<i64> {
    let server = store.add_server(root, "LUME_MINIFLUX_TOKEN")?;
    let feeds = ["Project journal", "Weekly notes"]
        .into_iter()
        .enumerate()
        .map(|(index, title)| RemoteFeedCandidate {
            remote_id: index as i64 + 1,
            title: title.into(),
            remote_url: format!("https://publisher.example/{index}/feed"),
            local_url: format!("{root}/v1/feeds/{}", index + 1),
        })
        .collect::<Vec<_>>();
    store.sync_server_feeds(server, &feeds)?;
    for feed in store.feeds()? {
        if let Some(remote) = &feed.server
            && remote.server_id == server
        {
            cache_article(store, &feed, server, remote.remote_id)?;
        }
    }
    store.set_server_enabled(server, !paused)?;
    Ok(server)
}

fn cache_article(
    store: &mut Store,
    feed: &Feed,
    server: i64,
    remote: i64,
) -> Result<()> {
    let article = IncomingArticle {
        external_id: format!("miniflux:{remote}"),
        title: format!("An article from {}", feed.title),
        url: format!("https://publisher.example/{server}/{remote}"),
        author: "Journal author".into(),
        published: 1_792_152_000,
        body: concat!(
            "# One local reading workspace\n\n",
            "Remote connections and direct feeds share a library.\n\n",
            "Cached articles, notes and reading progress remain available ",
            "while a connection is paused.",
        )
        .into(),
        body_origin: FeedOrigin::Feed,
    };
    let update = RemoteFeedUpdate {
        server_id: server,
        remote_id: remote,
        initialized: true,
        last_changed: 100,
        sync_after_id: 0,
        sync_latest: 100,
        states: Default::default(),
    };
    store.ingest_remote(feed, &feed.title, &[article], &update)
}
