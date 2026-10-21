//! Code panels fill the text viewport without changing copied source bytes.

mod common;

use anyhow::Result;
use lume::{
    app::{App, Focus},
    source, ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect, style::Color};

fn article(body: &str) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    store.ingest(&feed, &parsed.title, &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.workspace.fullscreen = true;
    app.focus = Focus::Articles;
    Ok((directory, app))
}

#[test]
fn fenced_code_has_one_solid_background_including_blank_and_short_rows()
-> Result<()> {
    let body = "```rust\nlet answer = 42;\n\nx\n```\n\nAfter with `inline`.";
    let (_directory, mut app) = article(body)?;
    for (theme, background, panel) in [
        ("lume", Color::Rgb(37, 35, 33), Color::Rgb(23, 23, 26)),
        ("nord", Color::Rgb(67, 76, 94), Color::Rgb(52, 60, 75)),
        ("gruvbox", Color::Rgb(60, 56, 54), Color::Rgb(40, 40, 40)),
    ] {
        app.workspace.theme = theme.into();
        for width in [50, 100, 160] {
            let area = Rect::new(0, 0, width, 30);
            let column = ui::workspace_reader_content(area, &app.workspace);
            app.resize_reader(column.width.into(), column.height.into());
            let mut terminal = Terminal::new(TestBackend::new(width, 30))?;

            terminal.draw(|frame| ui::draw(frame, &app))?;

            let buffer = terminal.backend().buffer();
            for row in 0..3 {
                for x in column.x..column.right() {
                    assert_eq!(
                        buffer[(x, column.y + row)].bg,
                        background,
                        "{theme} at {width}"
                    );
                }
            }
            assert_eq!(buffer[(column.right() - 1, column.y + 4)].bg, panel);
            assert_eq!(app.body, body);
            assert_eq!(app.reader_lines[0], "let answer = 42;");
        }
    }
    Ok(())
}

#[test]
fn standalone_code_uses_a_block_but_inline_fragments_keep_their_own_background()
-> Result<()> {
    let (_directory, mut app) =
        article("`standalone`\n\nText with `inline` code.")?;
    let area = Rect::new(0, 0, 100, 30);
    let column = ui::workspace_reader_content(area, &app.workspace);
    app.resize_reader(column.width.into(), column.height.into());
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    let buffer = terminal.backend().buffer();
    assert!(app.code_lines[0]);
    assert!(!app.code_lines[2]);
    assert_eq!(
        buffer[(column.right() - 1, column.y)].bg,
        Color::Rgb(37, 35, 33)
    );
    assert_eq!(
        buffer[(column.right() - 1, column.y + 2)].bg,
        Color::Rgb(23, 23, 26)
    );
    Ok(())
}

#[test]
fn code_inside_a_heading_retains_heading_presentation() {
    let rendered = lume::reader::render("# `Vec<T>`\n", 80);
    assert_eq!(rendered.lines[0].to_string(), "Vec<T>");
    assert!(!rendered.code[0]);
}

#[test]
fn tables_keep_the_uniform_panel_background_while_code_remains_distinct()
-> Result<()> {
    let (_directory, mut app) =
        article("| A | B |\n| --- | --- |\n| x | y |\n\n```c\nint n;\n```")?;
    let area = Rect::new(0, 0, 100, 30);
    let column = ui::workspace_reader_content(area, &app.workspace);
    app.resize_reader(column.width.into(), column.height.into());
    let mut terminal = Terminal::new(TestBackend::new(100, 30))?;

    terminal.draw(|frame| ui::draw(frame, &app))?;

    let buffer = terminal.backend().buffer();
    for row in app.table_rows[0].clone() {
        assert_eq!(
            buffer[(column.x + 2, column.y + row as u16)].bg,
            Color::Rgb(23, 23, 26)
        );
    }
    Ok(())
}
