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
    app.activate_pane(Focus::Reader);
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
    let reading = app.reader();
    let text = reading.reader_text.to_owned();
    let cursor = reading.cursor_offset();
    let selection = reading.selected_range();
    let source = (
        reading.content_source,
        reading.content_revision,
        reading.body.to_owned(),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.reader_pending() && Instant::now() < deadline {
        app.poll_reader();
        thread::sleep(Duration::from_millis(1));
    }
    ensure!(!app.reader_pending(), "Real syntax work did not complete");
    let reading = app.reader();
    assert_eq!(reading.reader_text, text);
    assert_eq!(reading.cursor_offset(), cursor);
    assert_eq!(reading.selected_range(), selection);
    assert_eq!(
        (
            reading.content_source,
            reading.content_revision,
            reading.body.to_owned()
        ),
        source
    );
    assert_eq!(
        app.reader()
            .reader_styled
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.style.fg == Some(Color::Rgb(216, 164, 116))),
        cfg!(feature = "syntax")
    );
    Ok(())
}

#[test]
fn help_documents_layout_and_all_later_sections_are_reachable() -> Result<()> {
    let (_directory, mut app) = application()?;
    keys(&mut app, ":help layout")?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert!(matches!(app.mode(), Mode::Help));
    let mut terminal = Terminal::new(TestBackend::new(120, 44))?;
    terminal.draw(|frame| ui::draw(frame, &app.presentation()))?;
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
    assert!(app.help_scroll() > 60);
    keys(&mut app, "q:help vim")?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert!(
        lume::app::HELP
            .lines()
            .nth(app.help_scroll())
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
    while app.navigation().navigation_loading || app.article_preparing() {
        app.poll_navigation()?;
        app.poll_reader();
        ensure!(Instant::now() < deadline, "Readable code did not arrive");
        thread::yield_now();
    }
    keys(&mut app, "2j0v3l")?;
    let selected = common::reader::selected_text(&app)?;
    app.resize_reader(45, 12);
    while app.reader_pending() {
        app.poll_reader();
        ensure!(
            Instant::now() < deadline,
            "Background colors did not arrive"
        );
        thread::yield_now();
    }
    assert_eq!(common::reader::selected_text(&app)?, selected);
    assert_eq!(
        app.reader()
            .reader_styled
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.style.fg == Some(Color::Rgb(216, 164, 116))),
        cfg!(feature = "syntax")
    );
    app.end_session()?;
    Ok(())
}

fn formula_application() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body = "# $x^2$\n\nBefore $x^2$.\n\nAnchor target after the formula.\n\nLast paragraph.".into();
    store.ingest(&feed, "Formula", &incoming.articles)?;
    let mut app = App::new(store, false)?;
    app.activate_pane(Focus::Reader);
    app.resize_reader(60, 8);
    let row = app
        .reader()
        .reader_lines
        .iter()
        .position(|row| row.contains("Anchor target"))
        .ok_or_else(|| anyhow::anyhow!("Source marker"))?;
    let byte = 0;
    common::reader::cursor(&mut app, row, byte)?;
    Ok((directory, app))
}

fn formula_command(app: &mut App, command: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(command)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

#[test]
fn formula_switch_reprojects_cached_text_and_preserves_source_cursor()
-> Result<()> {
    for background in [false, true] {
        let (directory, mut app) = formula_application()?;
        if background {
            app.enable_background_navigation(
                &directory.path().join("library.sqlite3"),
            )?;
        }
        let source = app.reader().body.to_owned();
        for command in [
            "latex-formulas off",
            "latex-formulas on",
            "latex-formulas off",
        ] {
            formula_command(&mut app, command)?;
            let deadline = Instant::now() + Duration::from_secs(5);
            while app.reader_pending() {
                app.poll_reader();
                ensure!(Instant::now() < deadline, "Formula reflow timed out");
                thread::sleep(Duration::from_millis(1));
            }
            assert_eq!(app.reader().body, source);
            assert!(
                app.reader().reader_lines[app.reader().reader_cursor]
                    .contains("Anchor target")
            );
            assert_eq!(
                app.reader().reader_text.contains("x²"),
                app.workspace().latex_formulas && cfg!(feature = "math")
            );
            assert_eq!(
                app.reader().reader_text.contains("$x^2$"),
                !app.workspace().latex_formulas || !cfg!(feature = "math")
            );
        }
        app.end_session()?;
    }
    Ok(())
}

#[test]
fn rapid_formula_toggles_discard_obsolete_background_layouts() -> Result<()> {
    let (directory, mut app) = formula_application()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    for command in [
        "latex-formulas off",
        "latex-formulas on",
        "latex-formulas off",
    ] {
        formula_command(&mut app, command)?;
    }
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.reader_pending() {
        app.poll_reader();
        ensure!(
            Instant::now() < deadline,
            "Latest formula policy not published"
        );
        thread::sleep(Duration::from_millis(1));
    }
    assert!(app.reader().reader_text.contains("$x^2$"));
    assert!(!app.reader().reader_text.contains("x²"));
    Ok(())
}

#[test]
fn invalid_formula_policy_preserves_the_previous_setting_and_source()
-> Result<()> {
    let (_directory, mut app) = formula_application()?;
    let text = app.reader().reader_text.to_owned();
    assert!(formula_command(&mut app, "latex-formulas nonsense").is_err());
    assert!(app.workspace().latex_formulas);
    assert_eq!(app.reader().reader_text, text);
    Ok(())
}

#[test]
fn formula_setting_has_a_reversible_live_preview() -> Result<()> {
    let (_directory, mut app) = formula_application()?;
    formula_command(&mut app, "settings")?;
    for _ in 0..13 {
        app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    }
    app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))?;
    assert!(!app.workspace().latex_formulas);
    assert!(app.reader().reader_text.contains("$x^2$"));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.workspace().latex_formulas);
    assert_eq!(
        app.reader().reader_text.contains("x²"),
        cfg!(feature = "math")
    );
    Ok(())
}
