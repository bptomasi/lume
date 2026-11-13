//! Visible-card clicks keep every pointer target stable through runtime frames.

use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use lume::frame_composition::draw_interactive;
use lume::{
    app::App,
    model::{Pane, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 160, 36);

pub(super) fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("cards.sqlite3");
    let mut store = Store::open(&path)?;
    super::super::seed(&mut store, if stress { 20_000 } else { 2000 })?;
    for article in store.library()?.articles.iter().take(32) {
        store.set_unread(article.id, false)?;
    }
    let mut app = App::new(store, false)?;
    app.set_view(View::All)?;
    app.enable_background_navigation(&path)?;
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    for compact in [false, true] {
        settle_current(&mut app)?;
        crate::fixture_toggle(&mut app, "compact", compact)?;
        app.resize_workspace(SCREEN);
        clicks(&mut app, &mut terminal, iterations)?;
    }
    app.end_session()
}

fn clicks(
    app: &mut App,
    terminal: &mut Terminal<TestBackend>,
    iterations: usize,
) -> Result<()> {
    let area = ui::workspace_areas(SCREEN, Pane::Articles, app.workspace())
        .articles
        .context("Articles hidden")?;
    let before = targets(app, area);
    let row_height = if app.workspace().compact { 2 } else { 4 };
    let bottom = before
        .iter()
        .rposition(Option::is_some)
        .context("No cards")?
        / row_height
        * row_height;
    super::super::measure(
        "visible article clicks + stable runtime frames",
        iterations * 20,
        || {
            for local in [bottom, 0] {
                app.mouse(MouseEvent {
                    kind: MouseEventKind::Down(MouseButton::Left),
                    column: area.x + 3,
                    row: area.y + 1 + local as u16,
                    modifiers: KeyModifiers::NONE,
                })?;
                app.resize_workspace(SCREEN);
                app.poll_reader();
                app.poll_navigation()?;
                terminal.draw(|frame| draw_interactive(frame, app))?;
                ensure!(
                    app.navigation().selected
                        == before[local].context("Missing card")?,
                    "Wrong card selected"
                );
                ensure!(
                    targets(app, area) == before,
                    "Click moved pointer targets"
                );
            }
            Ok(())
        },
    )
}

// Timed clicks admit background reads. Finish that selection before issuing
// the next preference command; setup must not act on a stale article.
fn settle_current(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.reader_stale()
        || app.reader_pending()
        || app.navigation().navigation_loading
    {
        ensure!(Instant::now() < deadline, "Card selection did not settle");
        app.poll_reader();
        app.poll_navigation()?;
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}

fn targets(app: &App, area: Rect) -> Vec<Option<usize>> {
    (area.y + 1..area.bottom() - 1)
        .map(|row| ui::article_at(app.navigation_hits(), area, row))
        .collect()
}
