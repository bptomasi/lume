//! Deferred syntax colors preserve usable text, selection and reader authority.

mod common;

use std::{
    thread,
    time::{Duration, Instant},
};

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    source, ui,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};

fn application() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body =
        "# Reader\n\n```rust\nfn main() { let n = 42; }\n```\n".into();
    store.ingest(&feed, "Journal", &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(60, 10);
    Ok((directory, app))
}

fn keys(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    Ok(())
}

#[test]
fn eventual_real_syntax_colors_preserve_selection_cursor_text_and_source()
-> Result<()> {
    let (_directory, mut app) = application()?;
    keys(&mut app, "2j0v3l")?;
    let before = (
        app.reader_text.clone(),
        app.cursor_offset(),
        app.selected_range(),
    );
    let source = (app.content_source, app.content_revision, app.body.clone());
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.reader_pending() && Instant::now() < deadline {
        app.poll_reader();
        thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.reader_pending(), "Real syntax work did not complete");
    assert_eq!(
        (
            app.reader_text.clone(),
            app.cursor_offset(),
            app.selected_range()
        ),
        before
    );
    assert_eq!(
        (app.content_source, app.content_revision, app.body.clone()),
        source
    );
    assert!(
        app.reader_styled
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.style.fg == Some(Color::Rgb(216, 164, 116)))
    );
    Ok(())
}

#[test]
fn help_documents_layout_and_all_later_sections_are_reachable() -> Result<()> {
    let (_directory, mut app) = application()?;
    keys(&mut app, ":help layout")?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert!(matches!(app.mode, Mode::Help));
    let mut terminal = Terminal::new(TestBackend::new(120, 44))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let visible = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(visible.contains(":settings"));
    assert!(visible.contains("Shift-Tab"));
    assert!(visible.contains(":panel-dock"));
    assert!(lume::app::HELP.contains(":sidebar-width"));
    keys(&mut app, "G")?;
    assert!(app.help_scroll > 60);
    keys(&mut app, "q:help vim")?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert!(
        lume::app::HELP
            .lines()
            .nth(app.help_scroll)
            .is_some_and(|line| line.contains("VIM"))
    );
    Ok(())
}

#[test]
fn background_markdown_and_syntax_keep_the_same_selected_code() -> Result<()> {
    let (directory, mut app) = application()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(lume::model::View::All)?;
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.navigation_loading || app.article_preparing() {
        app.poll_navigation()?;
        app.poll_reader();
        ensure!(Instant::now() < deadline, "Readable code did not arrive");
        thread::yield_now();
    }
    keys(&mut app, "2j0v3l")?;
    let selected = app.reader_text[app
        .selected_range()
        .ok_or_else(|| anyhow::anyhow!("No code selection"))?]
    .to_owned();
    app.resize_reader(45, 12);
    while app.reader_pending() {
        app.poll_reader();
        ensure!(
            Instant::now() < deadline,
            "Background colors did not arrive"
        );
        thread::yield_now();
    }
    assert_eq!(
        &app.reader_text[app
            .selected_range()
            .ok_or_else(|| anyhow::anyhow!("Selection lost"))?],
        selected
    );
    assert!(
        app.reader_styled
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.style.fg == Some(Color::Rgb(216, 164, 116)))
    );
    app.end_session()?;
    Ok(())
}
