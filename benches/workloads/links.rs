//! Structural indexing and exact cached Enter navigation, without network I/O.

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus},
    reader::index::DocumentIndex,
    source,
    store::Store,
};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let sections = if stress { 2000 } else { 200 };
    let body = fixture(sections);
    println!(
        "link index input · sections={sections} · bytes={}",
        body.len()
    );
    indexing(&body, iterations)?;
    navigation(&body, iterations)
}

fn indexing(body: &str, iterations: usize) -> Result<()> {
    let expected = body
        .match_indices("[jump-")
        .zip(body.match_indices("## Section "))
        .enumerate()
        .map(|(section, ((link, _), (anchor, _)))| {
            (link, format!("section-{section}"), anchor)
        })
        .collect::<Vec<_>>();
    super::measure(
        "link/anchor indexing + exact source hits",
        iterations,
        || {
            let index = DocumentIndex::parse(body);
            for (offset, fragment, anchor) in &expected {
                let link = index
                    .navigation
                    .link_at(*offset)
                    .ok_or_else(|| anyhow::anyhow!("Link missing"))?;
                ensure!(
                    link.destination.strip_prefix('#')
                        == Some(fragment.as_str()),
                    "Wrong occurrence"
                );
                ensure!(
                    index.navigation.anchor(fragment) == Some(*anchor),
                    "Wrong section"
                );
            }
            Ok(())
        },
    )?;
    Ok(())
}

fn fixture(sections: usize) -> String {
    let mut body = String::new();
    for section in 0..sections {
        body.push_str(&format!(
            "[jump-{section:04}](#section-{section})\n\n\
             ## Section {section}\n\nIndependent section contents.\n\n"
        ));
    }
    body
}

fn navigation(body: &str, iterations: usize) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("links.sqlite3"))?;
    store.subscribe("https://journal.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let mut parsed = source::parse(
        include_bytes!("../../tests/fixtures/rss.xml"),
        &feed.url,
    )?;
    parsed.articles[0].body = body.to_owned();
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(80, 24);
    super::measure(
        "cached Enter jump + history return",
        iterations * 20,
        || {
            let action =
                app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
            ensure!(action == Action::None, "Local jump launched a browser");
            ensure!(
                app.reader_lines[app.reader_cursor] == "Section 0",
                "Jump missed section"
            );
            app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
            ensure!(app.cursor_offset() == 0, "History lost link position");
            Ok(())
        },
    )
}
