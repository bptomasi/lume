//! Correctness-checked mouse/layout and article-note interaction workloads.

#[path = "pane_moves.rs"]
mod moves;

#[path = "article_cards.rs"]
mod article_cards;

use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use lume::{
    app::{Action, App, Focus, InputBatch},
    model::{FeedOrigin, IncomingArticle},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 144, 44);

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (_directory, mut app) = application(stress)?;
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    let count = app.annotations.len();
    println!("pane input: {} source bytes, {count} notes", app.body.len());
    wait_for_colors(&mut app)?;
    resize_workload(&mut app, &mut terminal, iterations)?;
    moves::run(&mut app, &mut terminal, iterations)?;
    selection_workload(&mut app, &mut terminal, iterations)?;
    command(&mut app, "annotations")?;
    let mut samples = Vec::new();
    for index in 0..iterations * 5 {
        app.focus = Focus::Annotations;
        let start = Instant::now();
        key(
            &mut app,
            if index % 2 == 0 {
                KeyCode::End
            } else {
                KeyCode::Home
            },
        )?;
        key(&mut app, KeyCode::Enter)?;
        frame(&mut app, &mut terminal)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        let expected = if index % 2 == 0 { count - 1 } else { 0 };
        ensure!(
            app.comments[app.comment_selected].body
                == format!("Insight {expected}"),
            "Annotation navigation lost source order"
        );
        ensure!(
            app.reader_lines[app.reader_cursor]
                .contains(&format!("Review concept {expected}.")),
            "Annotation jump did not reach its source paragraph"
        );
    }
    report("annotation navigation/jump + runtime frame", samples);
    article_cards::run(iterations, stress)
}

fn resize_workload(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let widths = (app.workspace.article_width, app.workspace.library_width);
    let text = app.reader_text.clone();
    let column = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?
        .x;
    let forward = divider_burst(column, 4);
    let backward = divider_burst(column + 4, -4);
    let mut samples = Vec::new();
    for _ in 0..iterations {
        let outward = forward.clone();
        let inward = backward.clone();
        let start = Instant::now();
        dispatch(app, outward)?;
        frame(app, terminal)?;
        dispatch(app, inward)?;
        frame(app, terminal)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        ensure!(
            (app.workspace.article_width, app.workspace.library_width)
                == widths,
            "Divider roundtrip lost saved dimensions"
        );
        ensure!(app.reader_text == text, "Reflow changed readable text");
    }
    report(
        "two divider drags (128 motion events) + reflow/frames",
        samples,
    );
    Ok(())
}

fn divider_burst(column: u16, delta: i32) -> Vec<Event> {
    let mut events =
        vec![mouse(MouseEventKind::Down(MouseButton::Left), column, 12)];
    events.extend((0..64).map(|step| {
        mouse(
            MouseEventKind::Drag(MouseButton::Left),
            (i32::from(column) + delta * (step % 4 + 1) / 4) as u16,
            12,
        )
    }));
    events.push(mouse(
        MouseEventKind::Up(MouseButton::Left),
        (i32::from(column) + delta) as u16,
        12,
    ));
    events
}

fn selection_workload(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let reader = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    let content = ui::workspace_reader_content(reader, &app.workspace);
    let row = app
        .reader_lines
        .iter()
        .position(|line| line.contains("café"))
        .context("Unicode fixture missing")? as u16
        + content.y;
    let mut samples = Vec::new();
    for _ in 0..iterations * 5 {
        let mut events = vec![mouse(
            MouseEventKind::Down(MouseButton::Left),
            content.x + 2,
            row,
        )];
        events.extend((0..64).map(|_| {
            mouse(MouseEventKind::Drag(MouseButton::Left), content.x + 13, row)
        }));
        events.push(mouse(
            MouseEventKind::Up(MouseButton::Left),
            content.x + 13,
            row,
        ));
        let start = Instant::now();
        dispatch(app, events)?;
        let copied = key(app, KeyCode::Char('y'))?;
        frame(app, terminal)?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        ensure!(
            copied == Action::Copy("café e\u{301} 界 🦀".into()),
            "Mouse yank changed bytes"
        );
    }
    report("Unicode selection burst/yank + runtime frame", samples);
    Ok(())
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("panes.sqlite3"))?;
    store.subscribe("https://benchmark.example/study")?;
    let feed = store.feeds()?.remove(0);
    let paragraphs = if stress { 500 } else { 50 };
    let marks = if stress { 200 } else { 20 };
    let body = document(paragraphs);
    store.ingest(
        &feed,
        "Study journal",
        &[IncomingArticle {
            external_id: "study".into(),
            title: "Read, understand and annotate".into(),
            url: "https://benchmark.example/study/article".into(),
            author: "Lume workload".into(),
            published: 2_000_000_000,
            body: body.clone(),
            body_origin: FeedOrigin::Feed,
        }],
    )?;
    let id = store.library()?.articles.remove(0).id;
    for index in (0..marks).rev() {
        let quote = format!("Review concept {index}.");
        let anchor = body.find(&quote).context("Missing source fixture")?;
        store.save_highlight(
            id,
            &quote,
            anchor,
            &format!("Insight {index}"),
        )?;
    }
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn document(paragraphs: usize) -> String {
    let mut body = "# Study in context\n\n".to_owned();
    for index in 0..paragraphs {
        body.push_str(&format!(
            concat!(
                "A café e\u{301} 界 🦀. Review concept {}. ",
                "A durable operation needs an explicit owner, a bounded retry ",
                "policy and a way to observe its outcome.\n\n"
            ),
            index
        ));
        if index % 10 == 0 {
            body.push_str("```rust\nfn owner() -> u32 { 42 }\n```\n\n");
        }
    }
    body
}

fn frame(app: &mut App, terminal: &mut Terminal<TestBackend>) -> Result<()> {
    app.resize_workspace(SCREEN);
    app.poll_reader();
    app.tick_pane_motion(Instant::now());
    terminal.draw(|frame| ui::draw_interactive(frame, app))?;
    Ok(())
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> Event {
    Event::Mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })
}

fn dispatch(app: &mut App, events: Vec<Event>) -> Result<()> {
    let mut batch = InputBatch::new(events);
    while let Some(action) = batch.dispatch_next(app)? {
        ensure!(action == Action::None, "Unexpected mouse side effect");
    }
    Ok(())
}

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<()> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)?;
    Ok(())
}

fn wait_for_colors(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.reader_pending() && Instant::now() < deadline {
        app.poll_reader();
        thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.reader_pending(), "Syntax worker deadline exceeded");
    Ok(())
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
