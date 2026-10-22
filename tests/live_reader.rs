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

fn formula_application() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].body = "# $x^2$\n\nBefore $x^2$.\n\nAnchor target after the formula.\n\nLast paragraph.".into();
    store.ingest(&feed, "Formula", &incoming.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(60, 8);
    app.reader_cursor = app
        .reader_lines
        .iter()
        .position(|row| row.contains("Anchor target"))
        .ok_or_else(|| anyhow::anyhow!("Source marker"))?;
    app.reader_column = 0;
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
        let source = app.body.clone();
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
            assert_eq!(app.body, source);
            assert!(
                app.reader_lines[app.reader_cursor].contains("Anchor target")
            );
            assert_eq!(
                app.reader_text.contains("x²"),
                app.workspace.latex_formulas
            );
            assert_eq!(
                app.reader_text.contains("$x^2$"),
                !app.workspace.latex_formulas
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
    assert!(app.reader_text.contains("$x^2$"));
    assert!(!app.reader_text.contains("x²"));
    Ok(())
}

#[test]
fn invalid_formula_policy_preserves_the_previous_setting_and_source()
-> Result<()> {
    let (_directory, mut app) = formula_application()?;
    let text = app.reader_text.clone();
    assert!(formula_command(&mut app, "latex-formulas nonsense").is_err());
    assert!(app.workspace.latex_formulas);
    assert_eq!(app.reader_text, text);
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
    assert!(!app.workspace.latex_formulas);
    assert!(app.reader_text.contains("$x^2$"));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.workspace.latex_formulas);
    assert!(app.reader_text.contains("x²"));
    Ok(())
}
