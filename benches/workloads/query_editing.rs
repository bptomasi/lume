//! Cursor-only query edits retain live results without reissuing storage work.

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    model::{FeedOrigin, IncomingArticle},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let articles = if stress { 20_000 } else { 2000 };
    let (directory, mut app) = application(articles)?;
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    let selected = app.current().context("Search result missing")?.id;
    println!("query editing input · articles={articles} · matches={articles}");
    super::measure(
        "query cursor round trip + cached frame",
        iterations * 20,
        || {
            for code in [KeyCode::Left, KeyCode::Right] {
                app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            ensure!(app.search == "systems", "Caret motion changed the query");
            ensure!(app.page_total == articles, "Live results changed");
            ensure!(matches!(app.mode, Mode::Input(_)), "Query editor closed");
            ensure!(
                app.current().map(|article| article.id) == Some(selected),
                "Caret motion changed the selected article"
            );
            Ok(())
        },
    )?;
    replacements(&mut app, directory.path(), articles, iterations)
}

fn replacements(
    app: &mut App,
    directory: &std::path::Path,
    count: usize,
    iterations: usize,
) -> Result<()> {
    app.key(KeyCode::Esc.into())?;
    app.enable_background_navigation(&directory.join("queries.sqlite3"))?;
    super::measure(
        "replaceable views: newest page and reader delivery",
        iterations.min(8),
        || {
            for view in [
                lume::model::View::Read,
                lume::model::View::All,
                lume::model::View::Inbox,
            ] {
                app.set_view(view)?;
            }
            let deadline =
                std::time::Instant::now() + std::time::Duration::from_secs(5);
            loop {
                app.poll_navigation()?;
                app.poll_reader();
                app.poll_note_workspace()?;
                if !app.navigation_loading && !app.reader_pending() {
                    break;
                }
                ensure!(
                    std::time::Instant::now() < deadline,
                    "Replacement did not finish"
                );
                std::thread::yield_now();
            }
            ensure!(app.page_total == count, "Newest page drifted");
            ensure!(
                app.reader_text.contains("A cached source"),
                "Newest source disappeared"
            );
            Ok(())
        },
    )
}

fn application(count: usize) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("queries.sqlite3"))?;
    store.subscribe("https://queries.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let articles = (0..count)
        .map(|index| IncomingArticle {
            external_id: index.to_string(),
            title: format!("Systems article {index}"),
            url: format!("https://queries.example/article/{index}"),
            author: "Fixture".into(),
            published: 1_800_000_000 + index as i64,
            body: "# Systems\n\nA cached source for query editing.".into(),
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    for batch in articles.chunks(2000) {
        store.ingest(&feed, "Queries", batch)?;
    }
    let mut app = App::new(store, false)?;
    app.focus = Focus::Articles;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE))?;
    app.paste("systems")?;
    Ok((directory, app))
}
