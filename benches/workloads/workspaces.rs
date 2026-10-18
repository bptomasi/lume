//! Input admission and completed navigation are timed separately and verified.

use std::time::{Duration, Instant};

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    model::{PanelLayout, SmartFilter, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("workspace-benchmark.sqlite3");
    let mut store = Store::open(&path)?;
    let count = if stress { 10_000 } else { 2_000 };
    super::seed(&mut store, count)?;
    seed_lists(&store)?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    let lists = app
        .library
        .smart_lists
        .iter()
        .map(|list| list.id)
        .collect::<Vec<_>>();
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    println!(
        "workspace input: {count} articles, {} smart lists",
        lists.len()
    );
    navigation(
        &mut app,
        &mut terminal,
        &lists,
        iterations,
        count,
        "synchronous",
    )?;
    app.enable_background_navigation(&path)?;
    navigation(
        &mut app,
        &mut terminal,
        &lists,
        iterations,
        count,
        "background",
    )?;
    bursts(&mut app, &mut terminal, &lists, iterations)?;
    layouts(&mut app, &mut terminal, iterations)?;
    app.end_session()?;
    super::workspace_reflow::run(iterations, stress)
}

fn seed_lists(store: &Store) -> Result<()> {
    for index in 0..8 {
        store.save_smart_list(
            &format!("Measured {index}"),
            &SmartFilter {
                text: if index % 2 == 0 {
                    "title:rust"
                } else {
                    "title:systems"
                }
                .into(),
                ..SmartFilter::default()
            },
        )?;
    }
    Ok(())
}

fn navigation(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    lists: &[i64],
    iterations: usize,
    count: usize,
    mode: &str,
) -> Result<()> {
    let mut admission = Vec::new();
    let mut complete = Vec::new();
    for _ in 0..iterations {
        for id in lists {
            let start = Instant::now();
            app.set_view(View::Smart(*id))?;
            terminal.draw(|frame| ui::draw_interactive(frame, app))?;
            admission.push(start.elapsed().as_secs_f64() * 1000.0);
            settle(app)?;
            terminal.draw(|frame| ui::draw_interactive(frame, app))?;
            complete.push(start.elapsed().as_secs_f64() * 1000.0);
            ensure!(app.page_total == count / 2, "Incomplete smart-list page");
            ensure!(
                app.body == include_str!("../../src/demo/systems.txt"),
                "Wrong article body"
            );
            ensure!(
                app.reader_text.contains("quiet elegance"),
                "Readable text missing"
            );
        }
    }
    report(&format!("{mode} navigation admission + frame"), admission);
    report(
        &format!("{mode} completed navigation + text/frame"),
        complete,
    );
    Ok(())
}

fn bursts(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    lists: &[i64],
    iterations: usize,
) -> Result<()> {
    let mut samples = Vec::new();
    for _ in 0..iterations {
        let start = Instant::now();
        for index in 0..200 {
            app.set_view(View::Smart(lists[index % lists.len()]))?;
        }
        terminal.draw(|frame| ui::draw_interactive(frame, app))?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        settle(app)?;
        ensure!(
            app.view == View::Smart(lists[199 % lists.len()]),
            "Burst restored a stale view"
        );
        ensure!(
            app.reader_text.contains("quiet elegance"),
            "Burst lost readable content"
        );
    }
    report("200 replaceable view requests + immediate frame", samples);
    Ok(())
}

fn layouts(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let mut samples = Vec::new();
    for _ in 0..iterations {
        let before = PanelLayout::capture(&app.workspace);
        let start = Instant::now();
        command(app, "workspace load Study")?;
        ensure!(app.workspace.layout.is_some(), "Study topology missing");
        app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
        terminal.draw(|frame| ui::draw_interactive(frame, app))?;
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
        command(app, "layout-undo")?;
        ensure!(
            PanelLayout::capture(&app.workspace) == before,
            "Layout undo changed preferences"
        );
        settle(app)?;
        app.flush_session()?;
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    report("Study profile + guide + undo + confirmed writes", samples);
    Ok(())
}

pub(super) fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(20);
    while app.navigation_loading || app.reader_pending() {
        app.poll_navigation()?;
        app.poll_reader();
        app.resize_workspace(Rect::new(0, 0, 144, 44));
        ensure!(Instant::now() < deadline, "Background work timed out");
        std::thread::yield_now();
    }
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    ensure!(
        !matches!(app.mode, lume::app::Mode::Input(_)),
        "Command left incomplete"
    );
    Ok(())
}

pub(super) fn report(label: &str, mut samples: Vec<f64>) {
    samples.sort_by(f64::total_cmp);
    let n = samples.len().saturating_sub(1);
    println!(
        "{label}: {} samples, p50={:.3} p95={:.3} p99={:.3} max={:.3} ms",
        samples.len(),
        samples[n / 2],
        samples[n * 95 / 100],
        samples[n * 99 / 100],
        samples[n]
    );
}
