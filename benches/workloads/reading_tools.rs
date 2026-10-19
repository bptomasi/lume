//! Correctness-checked cursor/clipboard/highlight and source-group workloads.

#[path = "trash.rs"]
mod trash;

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus},
    clipboard,
    model::{ArticleQuery, IncomingArticle, SmartFilter, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    super::recommendations::run(iterations, stress)?;
    selection(iterations, stress)?;
    super::note_interactions::run(iterations, stress)?;
    super::annotation_notebook::run(iterations, stress)?;
    source_group(iterations, stress)?;
    trash::run(iterations, stress)
}

fn selection(iterations: usize, stress: bool) -> Result<()> {
    let (_directory, mut app) = selection_app(stress)?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    selection_frames(&mut app, &mut terminal, iterations)?;
    horizontal_wrapping(&mut app, iterations)?;
    selection_history(&mut app, iterations)
}

fn horizontal_wrapping(app: &mut App, iterations: usize) -> Result<()> {
    super::measure(
        "cached grapheme movement across display rows",
        iterations * 50,
        || {
            keys(app, "gg$l")?;
            ensure!(
                (app.reader_cursor, app.reader_column) == (1, 0),
                "Forward motion stopped at the display row boundary"
            );
            keys(app, "h")?;
            ensure!(
                app.reader_cursor == 0
                    && app.reader_lines[0]
                        .get(app.reader_column..)
                        .is_some_and(|tail| tail == "."),
                "Backward motion failed to return to the previous row"
            );
            Ok(())
        },
    )
}

fn selection_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let content =
        ui::workspace_reader_content(Rect::new(0, 0, 144, 44), &app.workspace);
    super::measure(
        "precise selection + OSC52 + cached frame",
        iterations * 5,
        || {
            keys(app, "gg0wvll")?;
            ensure!(
                app.key(key('y'))? == Action::Copy("caf".into()),
                "Yank changed text"
            );
            let mut bytes = Vec::new();
            clipboard::copy(&mut bytes, "caf")?;
            ensure!(bytes == b"\x1b]52;c;Y2Fm\x1b\\", "Clipboard wire changed");
            terminal.draw(|frame| ui::draw(frame, app))?;
            ensure!(
                terminal.backend().buffer()[(content.x + 2, content.y)].bg
                    == ratatui::style::Color::Rgb(225, 179, 109),
                "Selection invisible"
            );
            ensure!(
                terminal.backend().buffer()[(content.x + 4, content.y)].bg
                    == ratatui::style::Color::Rgb(224, 219, 208),
                "Reader cursor lost contrast"
            );
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            Ok(())
        },
    )
}

fn selection_history(app: &mut App, iterations: usize) -> Result<()> {
    let notes = app.comments.len();
    super::measure(
        "durable precise highlight + reflow + undo",
        iterations,
        || {
            keys(app, "gg0wvlls")?;
            ensure!(app.comments.len() == notes + 1, "Highlight not saved");
            ensure!(
                app.comments.last().is_some_and(|note| note.quote == "caf"),
                "Wrong captured quotation"
            );
            keys(app, "u")?;
            ensure!(app.comments.len() == notes, "Undo lost prior highlights");
            Ok(())
        },
    )
}

fn selection_app(stress: bool) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("selection.db"))?;
    store.subscribe("https://benchmark.example/selection")?;
    let feed = store.feeds()?.remove(0);
    let paragraphs = if stress { 1000 } else { 200 };
    let marks = if stress { 200 } else { 20 };
    let body = (0..paragraphs)
        .map(|index| {
            format!("A café e\u{301} 界 🦀 reads original source {index}.\n\n")
        })
        .collect::<String>();
    store.ingest(&feed, "Reading tools", &[incoming(0, 0, body.clone())])?;
    let article = store.library()?.articles[0].id;
    for (offset, _) in body.match_indices("café").take(marks) {
        store.save_highlight(article, "café", offset, "")?;
    }
    println!(
        "selection input: {} bytes, {paragraphs} paragraphs, {marks} highlights",
        body.len()
    );
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    let content =
        ui::workspace_reader_content(Rect::new(0, 0, 144, 44), &app.workspace);
    app.resize_reader(content.width.into(), content.height.into());
    Ok((directory, app))
}

fn source_group(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("source-group.db"))?;
    let per_source = if stress { 1000 } else { 200 };
    let feeds = grouped_articles(&mut store, per_source)?;
    store.save_smart_list(
        "Security research",
        &SmartFilter {
            feeds: feeds.clone(),
            ..SmartFilter::default()
        },
    )?;
    let list = store.smart_lists()?.remove(0);
    let query = ArticleQuery {
        view: View::Smart(list.id),
        text: String::new(),
        offset: per_source * 4 - 20,
        limit: 20,
        retained: None,
    };
    let total = per_source * 8;
    println!("source-group input: {total} articles, 8 sources, 4 included");
    super::measure(
        "source-group count + oldest matching page",
        iterations,
        || {
            let page = store.query_articles(&query)?;
            ensure!(page.total == per_source * 4, "Source union count drifted");
            ensure!(page.articles.len() == 20, "Old articles inaccessible");
            ensure!(
                page.articles[0].title == "security 0:19",
                "Wrong oldest page"
            );
            ensure!(
                page.articles.iter().all(|a| feeds.contains(&a.feed_id)),
                "Unrelated source leaked into group"
            );
            Ok(())
        },
    )
}

fn grouped_articles(store: &mut Store, per_source: usize) -> Result<Vec<i64>> {
    let mut grouped = Vec::new();
    for source in 0..8 {
        let id = store
            .subscribe(&format!("https://benchmark.example/group/{source}"))?;
        let feed = store
            .feeds()?
            .into_iter()
            .find(|feed| feed.id == id)
            .ok_or_else(|| anyhow::anyhow!("Missing benchmark source"))?;
        let articles = (0..per_source)
            .map(|index| {
                incoming(
                    source,
                    index,
                    "Original vulnerability research.".into(),
                )
            })
            .collect::<Vec<_>>();
        store.ingest(&feed, &format!("Research {source}"), &articles)?;
        if source % 2 == 0 {
            grouped.push(id);
        }
    }
    Ok(grouped)
}

fn incoming(source: usize, index: usize, body: String) -> IncomingArticle {
    IncomingArticle {
        body_origin: lume::model::FeedOrigin::Feed,
        external_id: index.to_string(),
        title: format!("security {source}:{index}"),
        url: String::new(),
        author: "Benchmark author".into(),
        published: 2_000_000_000 + source as i64 * 10_000 + index as i64,
        body,
    }
}

fn key(character: char) -> KeyEvent {
    KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE)
}

fn keys(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        app.key(key(character))?;
    }
    Ok(())
}
