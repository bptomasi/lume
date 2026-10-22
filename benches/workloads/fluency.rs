//! Live search publication, searchable help and bounded formula preparation.
use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    model::{FeedOrigin, IncomingArticle},
    reader,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};
use std::time::{Duration, Instant};

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 20_000 } else { 2000 };
    let (_directory, mut app) = application(count)?;
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    println!(
        "live search · articles={count} · selected source={} bytes",
        app.body.len()
    );
    super::measure(
        "article query edits + metadata publication + frames",
        iterations,
        || {
            key(&mut app, KeyCode::Char('/'))?;
            for character in "systems article".chars() {
                key(&mut app, KeyCode::Char(character))?;
                settle(&mut app)?;
                terminal.draw(|frame| ui::draw(frame, &app))?;
                ensure!(!app.visible.is_empty(), "Live matches disappeared");
            }
            ensure!(app.page_total == count, "Search count changed");
            key(&mut app, KeyCode::Enter)?;
            settle(&mut app)?;
            key(&mut app, KeyCode::Esc)?;
            settle(&mut app)
        },
    )?;
    help(&mut app, &mut terminal, iterations)?;
    formulas(iterations, stress)
}

fn help(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    super::measure(
        "guide query + next/previous match + frame",
        iterations * 20,
        || {
            key(app, KeyCode::Char('?'))?;
            key(app, KeyCode::Char('/'))?;
            app.paste("resize")?;
            let first = app.help_scroll;
            key(app, KeyCode::Enter)?;
            key(app, KeyCode::Char('n'))?;
            ensure!(app.help_scroll != first, "Help match did not move");
            key(app, KeyCode::Char('N'))?;
            ensure!(app.help_scroll == first, "Help reverse lost its place");
            terminal.draw(|frame| ui::draw(frame, app))?;
            key(app, KeyCode::Esc)?;
            key(app, KeyCode::Esc)
        },
    )
}

fn formulas(iterations: usize, stress: bool) -> Result<()> {
    let count = if stress { 256 } else { 40 };
    let body = "Formula $R{\\mathcal{I}}=\\text{Combine}\\circ\\text{Disintegrate}$ and $x^2=\\frac{a}{b}$.\n\n".repeat(count);
    super::measure(
        "LaTeX source parse + Unicode projection + wrap",
        iterations,
        || {
            let rendered = reader::render(&body, 80);
            let converted = count.min(128);
            ensure!(
                rendered.text.matches("Combine ∘ Disintegrate").count()
                    == converted,
                "Formula budget/render mismatch"
            );
            ensure!(
                rendered.text.matches("x² = (a)/(b)").count() == converted,
                "Fraction lost"
            );
            Ok(())
        },
    )
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.navigation_loading {
        app.poll_reader();
        app.poll_navigation()?;
        ensure!(Instant::now() < deadline, "Search publication timed out");
        std::thread::yield_now();
    }
    Ok(())
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn application(count: usize) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let mut store = Store::open(&path)?;
    store.subscribe("https://fluency.example/feed")?;
    let feed = store.feeds()?.into_iter().next().context("Feed missing")?;
    let articles = (0..count)
        .map(|index| IncomingArticle {
            external_id: index.to_string(),
            title: format!("Systems article {index}"),
            url: format!("https://fluency.example/{index}"),
            author: "Fixture".into(),
            published: 1_800_000_000 + index as i64,
            body: if index + 1 == count {
                "Systems text for source preparation. ".repeat(3800)
            } else {
                "Systems source.".into()
            },
            body_origin: FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    for batch in articles.chunks(2000) {
        store.ingest(&feed, "Fluency", batch)?;
    }
    let mut app = App::new(store, false)?;
    app.focus = Focus::Articles;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    app.enable_background_navigation(&path)?;
    Ok((directory, app))
}
