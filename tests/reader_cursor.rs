//! High-contrast reader cells remain visible without a second terminal cursor.

mod common;

use std::{cell::RefCell, io::Write, rc::Rc};

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus},
    source, ui,
};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::{CrosstermBackend, TestBackend},
    layout::Rect,
    style::{Color, Modifier},
};

fn reader_app(body: &str) -> Result<(tempfile::TempDir, App, Rect)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    let content =
        ui::workspace_reader_content(Rect::new(0, 0, 100, 30), &app.workspace);
    app.resize_reader(content.width.into(), content.height.into());
    Ok((directory, app, content))
}

#[test]
fn cursor_is_an_ivory_block_distinct_from_amber_visual_selection() -> Result<()>
{
    let (_directory, mut app, content) = reader_app("abc")?;
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let first = &terminal.backend().buffer()[(content.x, content.y)];
    assert_eq!(first.symbol(), "a");
    assert_eq!(first.bg, Color::Rgb(224, 219, 208));
    assert_eq!(first.fg, Color::Rgb(18, 18, 20));
    assert!(first.modifier.contains(Modifier::BOLD));
    for key in ['v', 'l'] {
        app.key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE))?;
    }

    terminal.draw(|frame| ui::draw(frame, &app))?;

    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(content.x, content.y)].bg, Color::Rgb(225, 179, 109));
    assert_eq!(
        buffer[(content.x + 1, content.y)].bg,
        Color::Rgb(224, 219, 208)
    );
    Ok(())
}

#[test]
fn cursor_handles_wide_graphemes_and_empty_rows_without_coloring_unfocused_text()
-> Result<()> {
    let (_directory, mut app, content) = reader_app("界 text\n\nnext")?;
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    assert_eq!(
        terminal.backend().buffer()[(content.x, content.y)].symbol(),
        "界"
    );
    assert_eq!(
        terminal.backend().buffer()[(content.x, content.y)].bg,
        Color::Rgb(224, 219, 208)
    );
    app.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    assert_eq!(
        terminal.backend().buffer()[(content.x, content.y + 1)].bg,
        Color::Rgb(224, 219, 208)
    );
    app.focus = Focus::Articles;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    assert_ne!(
        terminal.backend().buffer()[(content.x, content.y + 1)].bg,
        Color::Rgb(224, 219, 208)
    );
    Ok(())
}

#[derive(Clone, Default)]
struct Capture(Rc<RefCell<Vec<u8>>>);

impl Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.borrow_mut().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[test]
fn crossterm_hides_the_hardware_cursor_while_drawing_and_restores_it_on_drop()
-> Result<()> {
    let (_directory, app, _content) = reader_app("Visible cursor")?;
    let capture = Capture::default();
    let backend = CrosstermBackend::new(capture.clone());
    let mut terminal = Terminal::with_options(
        backend,
        TerminalOptions {
            viewport: Viewport::Fixed(Rect::new(0, 0, 100, 30)),
        },
    )?;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    let during = String::from_utf8(capture.0.borrow().clone())?;
    assert!(
        during.contains("\x1b[?25l"),
        "Reader must hide the hardware cursor"
    );
    assert!(
        !during.contains("\x1b[?25h"),
        "A second cursor reverses the painted cell"
    );
    drop(terminal);
    let after = String::from_utf8(capture.0.borrow().clone())?;
    assert!(
        after.ends_with("\x1b[?25h"),
        "Cleanup must restore the shell cursor"
    );
    Ok(())
}
