//! Full cached frames and Unicode positioning isolate input-thread work.

use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, NavItem},
    model::{FeedOrigin, IncomingArticle, View},
    reader::cursor,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let (_directory, mut app) = application(stress)?;
    let original = app.reader_text.clone();
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    library_frames(&mut app, &mut terminal, iterations)?;
    selection_frames(&mut app, &mut terminal, iterations)?;
    ensure!(app.reader_text == original, "Input changed readable text");
    boundaries(iterations, stress)
}

fn library_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    app.focus = Focus::Library;
    let middle = app.navigation.len() / 2;
    super::measure(
        "library cursor + full interactive frame",
        iterations * 50,
        || {
            app.nav_selected = middle + app.nav_selected % 8;
            app.resize_workspace(Rect::new(0, 0, 144, 44));
            terminal.draw(|frame| ui::draw_interactive(frame, app))?;
            let area = ui::workspace_areas(
                Rect::new(0, 0, 144, 44),
                app.focus,
                &app.workspace,
            )
            .library
            .context("Library pane")?;
            let row = area.y + 1 + (area.height - 2) / 2;
            ensure!(
                ui::navigation_at(app, area, row) == Some(app.nav_selected),
                "Rendered navigation and mouse hit disagree"
            );
            app.nav_selected += 1;
            Ok(())
        },
    )
}

fn selection_frames(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    press(app, KeyCode::Char('G'))?;
    while app
        .reader_lines
        .get(app.reader_cursor)
        .is_some_and(|line| line.is_empty())
    {
        press(app, KeyCode::Char('k'))?;
    }
    press(app, KeyCode::Char('V'))?;
    let text = app.selected_range().context("Linewise selection")?;
    ensure!(
        app.reader_text[text.clone()].contains("final passage"),
        "Wrong final row: {:?}",
        &app.reader_text[text.clone()]
    );
    super::measure(
        "deep linewise selection + full interactive frame",
        iterations * 50,
        || {
            for key in ['k', 'j'] {
                press(app, KeyCode::Char(key))?;
                app.resize_workspace(Rect::new(0, 0, 144, 44));
                terminal.draw(|frame| ui::draw_interactive(frame, app))?;
            }
            ensure!(app.selected_range().is_some(), "Motion lost selection");
            Ok(())
        },
    )
}

fn boundaries(iterations: usize, stress: bool) -> Result<()> {
    let repeats = if stress { 16_384 } else { 2048 };
    let text = format!("{}x界", "column e\u{301} 🦀 ".repeat(repeats));
    let expected = text.len() - "界".len();
    super::measure("deep Unicode cursor boundary", iterations * 25, || {
        let actual = cursor::boundary(black_box(&text), text.len() - 2);
        ensure!(actual == expected, "Cursor split the final grapheme");
        Ok(())
    })
}

fn press(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn application(stress: bool) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("interaction.sqlite3"))?;
    store.subscribe("https://interaction.example/feed")?;
    let feed = store.feeds()?.remove(0);
    let repeats = if stress { 4000 } else { 400 };
    store.ingest(&feed, "Interaction", &[article(repeats)])?;
    let mut app = App::new(store, false)?;
    let destinations = if stress { 4096 } else { 128 };
    app.navigation
        .extend((0..destinations).map(|index| NavItem {
            view: View::Feed(index as i64 + 100),
            title: format!("Engineering source {index}"),
            count: index % 200,
            group: if index < destinations / 2 {
                "FEEDS"
            } else {
                "SMART LISTS"
            },
        }));
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.reader_pending() && Instant::now() < deadline {
        app.poll_reader();
        std::thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.reader_pending(), "Reader fixture did not settle");
    println!(
        "interaction: {destinations} projected destinations, {repeats} paragraphs"
    );
    Ok((directory, app))
}

fn article(repeats: usize) -> IncomingArticle {
    IncomingArticle {
        external_id: "cached".into(),
        title: "Measured article".into(),
        url: "https://interaction.example/article".into(),
        author: "Fixture".into(),
        published: 1_800_000_000,
        body: format!(
            "# Reading\n\n{}A final passage.\n",
            "A café 🦀 with enough detail to study.\n\n".repeat(repeats)
        ),
        body_origin: FeedOrigin::Feed,
    }
}
