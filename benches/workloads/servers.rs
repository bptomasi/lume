//! Representative cached server navigation with independent count checks.

use anyhow::{Context, Result, ensure};
use lume::{
    app::App,
    model::{
        FeedOrigin, IncomingArticle, RemoteFeedCandidate, RemoteFeedUpdate,
        View,
    },
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let count = if stress { 1000 } else { 100 };
    let server = seed(&mut store, count)?;
    let mut app = App::new(store, false)?;
    let mut terminal = Terminal::new(TestBackend::new(120, 40))?;
    super::measure(
        "cached server catalog + page + fold/unfold frames",
        iterations * 5,
        || {
            app.reload()?;
            app.set_view(View::Server(server))?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            ensure!(app.library.servers.len() == 1, "Server disappeared");
            ensure!(app.page_total == count, "Aggregate count changed");
            ensure!(
                app.navigation
                    .iter()
                    .filter(|item| item.group == "SERVERS"
                        && !matches!(item.view, View::Group(_)))
                    .count()
                    == count + 1,
                "Remote destinations missing"
            );
            ensure!(
                !app.navigation.iter().any(|item| item.group == "FEEDS"),
                "Remote feeds leaked into local subscriptions"
            );
            verify_section_fold(&mut app, &mut terminal)?;
            Ok(())
        },
    )
}

fn verify_section_fold(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
) -> Result<()> {
    app.focus = lume::app::Focus::Library;
    app.set_view(View::Group("SERVERS".into()))?;
    app.key(crossterm::event::KeyCode::Enter.into())?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    ensure!(
        app.navigation
            .iter()
            .filter(|item| item.group == "SERVERS")
            .count()
            == 1,
        "Folded section retained children"
    );
    app.key(crossterm::event::KeyCode::Enter.into())?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    ensure!(
        app.navigation
            .iter()
            .any(|item| matches!(item.view, View::Server(_))),
        "Unfolding lost the connection"
    );
    Ok(())
}

fn seed(store: &mut Store, count: usize) -> Result<i64> {
    let server = store.add_server("https://reader.example", "LUME_TOKEN")?;
    let candidates = (1..=count)
        .map(|index| RemoteFeedCandidate {
            remote_id: index as i64,
            title: format!("Remote feed {index}"),
            remote_url: format!("https://publisher.example/{index}"),
            local_url: format!("https://reader.example/v1/feeds/{index}"),
        })
        .collect::<Vec<_>>();
    store.sync_server_feeds(server, &candidates)?;
    for feed in store.feeds()? {
        let remote = feed.server.as_ref().context("Remote identity")?;
        let article = IncomingArticle {
            external_id: format!("miniflux:{}", remote.remote_id),
            title: feed.title.clone(),
            url: format!(
                "https://publisher.example/articles/{}",
                remote.remote_id
            ),
            author: String::new(),
            published: 1_800_000_000,
            body: "Cached article with preserved personal reading state."
                .into(),
            body_origin: FeedOrigin::Feed,
        };
        let update = RemoteFeedUpdate {
            history: Default::default(),
            server_id: server,
            remote_id: remote.remote_id,
            initialized: true,
            last_changed: 100,
            sync_after_id: 0,
            sync_latest: 100,
            states: Default::default(),
        };
        store.ingest_remote(&feed, &feed.title, &[article], &update)?;
    }
    Ok(server)
}
