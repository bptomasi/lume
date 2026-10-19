//! Actual Ratatui buffers qualify layout and modal content at several sizes.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    demo,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};

#[test]
fn three_panes_have_dark_background_and_selected_article_content() -> Result<()>
{
    let (_directory, mut app) = demo_app()?;
    let mut terminal = Terminal::new(TestBackend::new(144, 44))?;
    app.resize_reader(78, 29);
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let buffer = terminal.backend().buffer();
    let text = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("01  Library"));
    assert!(text.contains("02  Articles"));
    assert!(text.contains("03  Reader"));
    assert!(text.contains("The quiet elegance of doing less"));
    assert!(text.contains("NORMAL"));
    for position in [(0, 0), (0, 1), (0, 43), (1, 3)] {
        assert_eq!(buffer[position].bg, Color::Rgb(23, 23, 26));
    }
    Ok(())
}

#[test]
fn compact_and_tiny_terminals_render_every_mode_without_overflow() -> Result<()>
{
    let (_directory, mut app) = demo_app()?;
    for (width, height) in
        [(1, 1), (23, 9), (40, 12), (80, 24), (119, 30), (160, 50)]
    {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        for focus in [Focus::Library, Focus::Articles, Focus::Reader] {
            app.focus = focus;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.key(KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE))?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
            app.focus = Focus::Reader;
            app.paste("note")?;
            app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
            app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
            app.paste("q!")?;
            app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
        }
    }
    Ok(())
}

fn demo_app() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("demo.sqlite3"))?;
    demo::seed(&mut store)?;
    Ok((directory, App::new(store, true)?))
}

#[test]
fn selectors_zen_and_smart_forms_remain_usable_at_small_sizes() -> Result<()> {
    let (_directory, mut app) = demo_app()?;
    for (width, height) in [(24, 10), (40, 12), (80, 24), (144, 44)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        for sequence in [" ", " p", " f", " l", " s", ":", ":fold", "z"] {
            app.mode = lume::app::Mode::Normal;
            for character in sequence.chars() {
                app.key(KeyEvent::new(
                    KeyCode::Char(character),
                    KeyModifiers::NONE,
                ))?;
            }
            terminal.draw(|frame| ui::draw(frame, &app))?;
            if matches!(app.mode, lume::app::Mode::Smart(_)) {
                for _ in 0..6 {
                    app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE))?;
                    terminal.draw(|frame| ui::draw(frame, &app))?;
                }
            }
            app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
        }
    }
    Ok(())
}

#[test]
fn command_menu_exposes_syntax_and_filters_visible_choices() -> Result<()> {
    let (_directory, mut app) = demo_app()?;
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains(":folder NAME"));
    assert!(text.contains("Tab complete"));
    for character in "fold".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let text = terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert!(text.contains("2 matches"));
    assert!(text.contains(":folders"));
    assert!(!text.contains(":refresh"));
    Ok(())
}
