//! Read-only Vim workflows through the application controller and real layout.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus, InputBatch, Mode},
    source,
    store::Store,
};

fn reading(code: &str) -> Result<(tempfile::TempDir, App)> {
    reading_body(&format!("```text\n{code}\n```\n"))
}

fn reading_body(body: &str) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    store.ingest(&feed, "Journal", &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_reader(70, 5);
    Ok((directory, app))
}

fn keys(app: &mut App, text: &str) -> Result<Action> {
    let mut action = Action::None;
    for character in text.chars() {
        action = app
            .key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    Ok(action)
}

fn command(app: &mut App, text: &str) -> Result<()> {
    keys(app, ":")?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

#[test]
fn word_word_end_and_word_objects_distinguish_punctuation_and_whitespace()
-> Result<()> {
    let (_directory, mut app) = reading("alpha.beta foo_bar e\u{301}lan 👩‍💻")?;
    for (sequence, column) in [
        ("w", 5),
        ("w", 6),
        ("e", 9),
        ("W", 11),
        ("E", 17),
        ("b", 11),
        ("ge", 9),
        ("B", 0),
        ("ge", 0),
        ("2W", 19),
    ] {
        keys(&mut app, sequence)?;
        assert_eq!(app.reader_column, column, "{sequence}");
    }
    keys(&mut app, "gg0viw")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("alpha".into()));
    app.selection = None;
    keys(&mut app, "gg0viW")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("alpha.beta".into()));
    app.selection = None;
    keys(&mut app, "gg0vaw")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("alpha".into()));
    Ok(())
}

#[test]
fn counts_rows_edges_and_unicode_movement_leave_publisher_text_unchanged()
-> Result<()> {
    let (_directory, mut app) = reading("  alpha  \nβeta\n界 👩‍💻\nlast")?;
    let original = app.body.clone();
    keys(&mut app, "^g_")?;
    assert_eq!(app.reader_column, 6);
    keys(&mut app, "3G0l")?;
    assert_eq!(app.reader_cursor, 2);
    assert_eq!(app.reader_column, "界".len());
    keys(&mut app, "2h")?;
    assert_eq!((app.reader_cursor, app.reader_column), (1, 4));
    keys(&mut app, "2gg")?;
    assert_eq!(app.reader_cursor, 1);
    keys(&mut app, "2j")?;
    assert_eq!(app.reader_cursor, 3);
    keys(&mut app, "gg2j")?;
    assert_eq!(app.reader_cursor, 2);
    assert_eq!(app.body, original);
    assert!(matches!(app.mode, Mode::Normal));
    Ok(())
}

#[test]
fn horizontal_motions_cross_soft_wraps_and_yank_the_intervening_space()
-> Result<()> {
    let (_directory, mut app) =
        reading_body("alpha beta gamma delta epsilon zeta")?;
    app.resize_reader(12, 2);
    assert_eq!(&app.reader_lines[..2], ["alpha beta", "gamma delta"]);
    let original = app.body.clone();

    keys(&mut app, "$vl")?;

    assert_eq!((app.reader_cursor, app.reader_column), (1, 0));
    assert_eq!(keys(&mut app, "y")?, Action::Copy("a g".into()));
    app.selection = None;
    app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE))?;
    assert_eq!((app.reader_cursor, app.reader_column), (0, 9));
    app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::NONE))?;
    assert_eq!((app.reader_cursor, app.reader_column), (1, 0));
    keys(&mut app, "$l")?;
    assert_eq!((app.reader_cursor, app.reader_column), (2, 0));
    assert!(app.scroll > 0);
    assert!(app.reader_cursor < app.scroll + app.reader_height);
    assert_eq!(app.body, original);
    Ok(())
}

#[test]
fn counted_horizontal_motions_preserve_graphemes_and_visit_empty_rows()
-> Result<()> {
    let (_directory, mut app) = reading("e\u{301}👩‍💻\n\n界x")?;

    keys(&mut app, "l")?;
    assert_eq!((app.reader_cursor, app.reader_column), (0, 3));
    keys(&mut app, "l")?;
    assert_eq!((app.reader_cursor, app.reader_column), (1, 0));
    keys(&mut app, "2l")?;
    assert_eq!((app.reader_cursor, app.reader_column), (2, 3));
    keys(&mut app, "h")?;
    assert_eq!((app.reader_cursor, app.reader_column), (2, 0));
    keys(&mut app, "h")?;
    assert_eq!((app.reader_cursor, app.reader_column), (1, 0));
    keys(&mut app, "h")?;
    assert_eq!((app.reader_cursor, app.reader_column), (0, 3));
    keys(&mut app, "gg04l")?;
    assert_eq!((app.reader_cursor, app.reader_column), (2, 3));
    keys(&mut app, "G$")?;
    let end = (app.reader_cursor, app.reader_column);
    keys(&mut app, "9999l")?;
    assert_eq!((app.reader_cursor, app.reader_column), end);
    keys(&mut app, "gg09999h")?;
    assert_eq!((app.reader_cursor, app.reader_column), (0, 0));
    Ok(())
}

#[test]
fn character_finds_till_repeats_and_reverse_never_toggle_favorites()
-> Result<()> {
    let (directory, mut app) = reading("a:x b:x c:x")?;
    keys(&mut app, "tx")?;
    assert_eq!(app.reader_column, 1);
    keys(&mut app, ";")?;
    assert_eq!(app.reader_column, 5);
    keys(&mut app, ",")?;
    assert_eq!(app.reader_column, 3);
    keys(&mut app, "0fx")?;
    assert_eq!(app.reader_column, 2);
    keys(&mut app, "2fx")?;
    assert_eq!(app.reader_column, 10);
    let stored = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(
        !stored
            .article(app.current().context("Missing article")?.id)?
            .starred
    );
    command(&mut app, "favorite")?;
    assert!(
        stored
            .article(app.current().context("Missing article")?.id)?
            .starred
    );
    Ok(())
}

#[test]
fn visual_find_and_precise_yank_keep_the_selected_source_bytes() -> Result<()> {
    let (_directory, mut app) = reading("call(value + other); tail")?;
    keys(&mut app, "vfe")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("call(value".into()));
    app.selection = None;
    keys(&mut app, "gg0%")?;
    assert_eq!(app.reader_column, 18);
    keys(&mut app, "%")?;
    assert_eq!(app.reader_column, 4);
    Ok(())
}

#[test]
fn a_count_is_honored_by_batched_input_and_escape_cancels_only_the_prefix()
-> Result<()> {
    let (_directory, mut app) = reading("one\ntwo\nthree\nfour\nfive")?;
    let events = "3j"
        .chars()
        .map(|character| {
            Event::Key(KeyEvent::new(
                KeyCode::Char(character),
                KeyModifiers::NONE,
            ))
        })
        .collect();
    let mut batch = InputBatch::new(events);
    while batch.dispatch_next(&mut app)?.is_some() {}
    assert_eq!(app.reader_cursor, 3);
    app.workspace.fullscreen = true;
    keys(&mut app, "7f")?;
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.workspace.fullscreen);
    keys(&mut app, "k")?;
    assert_eq!(app.reader_cursor, 2);
    Ok(())
}

#[test]
fn reader_search_shortcuts_and_note_commands_keep_their_distinct_contexts()
-> Result<()> {
    let (_directory, mut app) = reading("alpha beta alpha")?;
    command(&mut app, "find alpha")?;
    let first = app.reader_column;
    keys(&mut app, "n")?;
    assert_ne!(app.reader_column, first);
    keys(&mut app, "N")?;
    assert_eq!(app.reader_column, first);
    command(&mut app, "note")?;
    assert!(matches!(app.mode, Mode::Input(_)));
    app.paste("A private note")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    command(&mut app, "edit-note")?;
    assert!(matches!(app.mode, Mode::Input(_)));
    Ok(())
}

#[test]
fn character_find_targets_take_priority_over_visual_and_yank_shortcuts()
-> Result<()> {
    let (_directory, mut app) = reading("a v y s")?;
    for (sequence, column) in [("fv", 2), ("fy", 4), ("fs", 6)] {
        keys(&mut app, sequence)?;
        assert_eq!(app.reader_column, column);
        assert!(app.selection.is_none());
        assert!(app.comments.is_empty());
        assert!(matches!(app.mode, Mode::Normal));
    }
    keys(&mut app, "0vfy")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("a v y".into()));
    Ok(())
}

#[test]
fn word_objects_include_the_previous_word_when_on_trailing_whitespace()
-> Result<()> {
    let (_directory, mut app) = reading("one two  ")?;
    keys(&mut app, "$vaw")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("two  ".into()));
    Ok(())
}

#[test]
fn counted_pages_viewport_motions_and_positioning_keep_the_cursor_visible()
-> Result<()> {
    let rows = (0..30)
        .map(|row| format!("line {row}\n"))
        .collect::<String>();
    let (_directory, mut app) = reading(&rows)?;
    for (character, row) in [('d', 2), ('f', 7), ('b', 2), ('u', 0)] {
        app.key(KeyEvent::new(
            KeyCode::Char(character),
            KeyModifiers::CONTROL,
        ))?;
        assert_eq!(app.reader_cursor, row);
    }
    keys(&mut app, "15Gzt")?;
    assert_eq!(app.scroll, 14);
    keys(&mut app, "zz")?;
    assert_eq!(app.scroll, 12);
    keys(&mut app, "zb")?;
    assert_eq!(app.scroll, 10);
    for (sequence, row) in [("H", 10), ("M", 12), ("L", 14)] {
        keys(&mut app, sequence)?;
        assert_eq!(app.reader_cursor, row);
    }
    keys(&mut app, "2")?;
    app.key(KeyEvent::new(KeyCode::Char('e'), KeyModifiers::CONTROL))?;
    assert_eq!(app.scroll, 12);
    assert_eq!(app.reader_cursor, 14);
    keys(&mut app, "2")?;
    app.key(KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL))?;
    assert_eq!(app.reader_cursor, 4);
    Ok(())
}

#[test]
fn uppercase_edges_and_column_motion_extend_selection_without_editing()
-> Result<()> {
    let (_directory, mut app) = reading("\t  café 界\nnext row\n")?;
    let body = app.body.clone();
    keys(&mut app, "I")?;
    assert_eq!(app.reader_column, 3);
    keys(&mut app, "vA")?;
    assert_eq!(keys(&mut app, "y")?, Action::Copy("café 界".into()));
    app.selection = None;
    keys(&mut app, "0g^")?;
    assert_eq!(app.reader_column, 3);
    keys(&mut app, "5|")?;
    assert_eq!(app.reader_column, 1);
    keys(&mut app, "9|")?;
    assert_eq!(app.reader_column, 5);
    assert_eq!(app.body, body);
    assert!(matches!(app.mode, Mode::Normal));
    Ok(())
}

#[test]
fn control_word_and_document_aliases_are_reader_local() -> Result<()> {
    let (_directory, mut app) = reading("alpha beta\nlast row")?;
    let last_row = app.reader_lines.len() - 1;
    for (code, row, column) in [
        (KeyCode::Right, 0, 6),
        (KeyCode::Left, 0, 0),
        (KeyCode::End, last_row, 0),
        (KeyCode::Home, 0, 0),
    ] {
        app.key(KeyEvent::new(code, KeyModifiers::CONTROL))?;
        assert_eq!((app.reader_cursor, app.reader_column), (row, column));
    }
    app.focus = Focus::Articles;
    app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::CONTROL))?;
    assert_eq!(app.focus, Focus::Articles);
    Ok(())
}
