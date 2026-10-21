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
    let (_directory, mut app) = application(articles)?;
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
