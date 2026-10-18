//! Interactive frames include runtime resize/poll work and cold smart lists.

use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    model::{FeedOrigin, IncomingArticle, SmartFilter, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (_directory, mut app) = application(stress)?;
    let lists = app
        .library
        .smart_lists
        .iter()
        .map(|list| list.id)
        .collect::<Vec<_>>();
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    app.focus = Focus::Library;
    let mut samples = Vec::new();
    for _ in 0..iterations {
        for id in &lists {
            let start = Instant::now();
            app.set_view(View::Smart(*id))?;
            frame(&mut app, &mut terminal)?;
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            ensure!(
                app.page_total == 100,
                "Smart list {id} expected 100 articles, got {} in {:?}",
                app.page_total,
                app.view
            );
            ensure!(
                app.reader_text.contains("value"),
                "Plain code unavailable"
            );
        }
    }
    report("smart-list change + text-first frame", samples);
    finish_colors(&mut app)?;
    reader_frames(&mut app, &mut terminal, iterations)
}

fn reader_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    let mut samples = Vec::new();
    let original = app.reader_text.clone();
    for _ in 0..iterations * 10 {
        for character in "gg3WgeE0jklb$".chars() {
            let start = Instant::now();
            app.key(KeyEvent::new(
                KeyCode::Char(character),
                KeyModifiers::NONE,
            ))?;
            frame(app, terminal)?;
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
            ensure!(
                app.cursor_offset() <= app.reader_text.len(),
                "Invalid cursor"
            );
            ensure!(
                app.reader_text.is_char_boundary(app.cursor_offset()),
                "Split UTF-8"
            );
        }
    }
    ensure!(
        app.reader_text == original,
        "Motion modified readable content"
    );
    report("Vim input + runtime resize/poll + frame", samples);
    Ok(())
}

fn frame(app: &mut App, terminal: &mut Terminal<TestBackend>) -> Result<()> {
    let size = terminal.size()?;
    let area = ratatui::layout::Rect::new(0, 0, size.width, size.height);
    let areas = ui::workspace_areas(area, app.focus, &app.workspace);
    if let Some(area) = areas.reader {
        let content = ui::workspace_reader_content(area, &app.workspace);
        app.resize_reader(content.width.into(), content.height.into());
    }
    app.poll_reader();
    terminal.draw(|frame| ui::draw(frame, app))?;
    Ok(())
}

fn finish_colors(app: &mut App) -> Result<()> {
    let start = Instant::now();
    let deadline = start + Duration::from_secs(10);
    while app.reader_pending() && Instant::now() < deadline {
        app.poll_reader();
        thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.reader_pending(), "Syntax worker deadline exceeded");
    ensure!(
        app.reader_styled
            .iter()
            .zip(&app.code_lines)
            .filter(|(_, code)| **code)
            .map(|(line, _)| line)
            .flat_map(|line| &line.spans)
            .any(|span| span.style.fg.is_some()),
        "Syntax colors not delivered"
    );
    println!(
        "remaining syntax delivery: {:.3} ms",
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("smoothness.db"))?;
    let count = if stress { 20_000 } else { 2000 };
    let feed_id = store.subscribe("https://benchmark.example/smoothness")?;
    let feed = store.feeds()?.remove(0);
    for start in (0..count).step_by(1000) {
        let articles = (start..(start + 1000).min(count))
            .map(article)
            .collect::<Vec<_>>();
        store.ingest(&feed, "Measured computing", &articles)?;
    }
    for index in 0..count / 100 {
        store.save_smart_list(
            &format!("Scope {index}"),
            &SmartFilter {
                text: format!("title:\"scope{index}\""),
                feeds: vec![feed_id],
                ..SmartFilter::default()
            },
        )?;
    }
    println!(
        "smoothness input: {count} articles, {} smart lists",
        count / 100
    );
    Ok((directory, App::new(store, false)?))
}

fn article(index: usize) -> IncomingArticle {
    let language = ["rust", "go", "sql", "typescript"][index / 100 % 4];
    let code = [
        "fn value() -> u32 { 42 }",
        "func value() int { return 42 }",
        "SELECT 42 AS value;",
        "const value: number = 42;",
    ][index / 100 % 4];
    IncomingArticle {
        body_origin: FeedOrigin::Feed,
        external_id: index.to_string(),
        title: format!("scope{} reading {index}", index / 100),
        url: format!("https://benchmark.example/reading/{index}"),
        author: "Benchmark author".into(),
        published: 2_000_000_000 + index as i64,
        body: format!(
            "# Reading {index}\n\nA café 🦀.\n\n```{language}\n{code}\n```\n\nNext paragraph.\n"
        ),
    }
}

fn report(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let at = |percent: usize| samples[(samples.len() - 1) * percent / 100];
    println!(
        "{label}: {} samples, p50={:.3} p95={:.3} p99={:.3} max={:.3} ms",
        samples.len(),
        at(50),
        at(95),
        at(99),
        at(100)
    );
}
