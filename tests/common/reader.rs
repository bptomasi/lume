//! Reader scenarios enter through ordinary keys, preserving cursor invariants.

use anyhow::{Context, Result};
use crossterm::event::KeyCode;
use lume::app::{App, Focus};

fn counted(app: &mut App, count: usize, command: char) -> Result<()> {
    for digit in count.max(1).to_string().chars() {
        app.key(KeyCode::Char(digit).into())?;
    }
    app.key(KeyCode::Char(command).into())?;
    Ok(())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn cursor(app: &mut App, row: usize, byte: usize) -> Result<()> {
    app.activate_pane(Focus::Reader);
    counted(app, row.saturating_add(1).min(9999), 'G')?;
    let text = app
        .reader()
        .reader_lines
        .get(app.reader().reader_cursor)
        .map_or("", String::as_str);
    let end = lume::reader::cursor::previous(text, text.len());
    let column = lume::reader::cursor::column(text, byte);
    if byte > 0 && byte >= end {
        app.key(KeyCode::End.into())?;
    } else if column > 0 {
        counted(app, column.saturating_add(1), '|')?;
    } else {
        app.key(KeyCode::Home.into())?;
    }
    Ok(())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn scroll(app: &mut App, row: usize) -> Result<()> {
    cursor(app, row, 0)?;
    app.key(KeyCode::Char('z').into())?;
    app.key(KeyCode::Char('t').into())?;
    Ok(())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn horizontal(app: &mut App, offset: usize) -> Result<()> {
    let current = app.reader().horizontal;
    let (command, distance) = if offset >= current {
        ('l', offset - current)
    } else {
        ('h', current - offset)
    };
    if distance > 0 {
        for digit in distance.to_string().chars() {
            app.key(KeyCode::Char(digit).into())?;
        }
        app.key(KeyCode::Char('z').into())?;
        app.key(KeyCode::Char(command).into())?;
    }
    Ok(())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn select(app: &mut App, anchor: usize) -> Result<()> {
    let row = app.reader().reader_cursor;
    let column = app.reader().reader_column;
    let anchor_row = app
        .reader()
        .reader_ranges
        .partition_point(|range| range.start <= anchor)
        .saturating_sub(1);
    let anchor_column = app
        .reader()
        .reader_ranges
        .get(anchor_row)
        .map_or(0, |range| anchor.saturating_sub(range.start));
    app.clear_copied_selection();
    cursor(app, anchor_row, anchor_column)?;
    app.key(KeyCode::Char('v').into())?;
    cursor(app, row, column)?;
    Ok(())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn comment(app: &mut App, index: usize) -> Result<()> {
    let row = app.reader().reader_cursor;
    let byte = app.reader().reader_column;
    let focus = app.focus();
    let count = app.reader().comments.len();
    anyhow::ensure!(index < count, "Fixture comment is missing");
    for _ in 0..count {
        if app.reader().comment_selected == index {
            break;
        }
        app.key(KeyCode::Char(']').into())?;
    }
    anyhow::ensure!(
        app.reader().comment_selected == index,
        "Comment navigation did not select the fixture note"
    );
    cursor(app, row, byte)?;
    app.activate_pane(focus);
    Ok(())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn selected_text(app: &App) -> Result<String> {
    let reading = app.reader();
    let selected = reading
        .selected_range()
        .context("Reader selection missing")?;
    Ok(reading.reader_text[selected].to_owned())
}

#[allow(dead_code, reason = "Each target uses its own reader scenarios")]
pub fn position(app: &App) -> (usize, usize) {
    let reading = app.reader();
    (reading.reader_cursor, reading.reader_column)
}
