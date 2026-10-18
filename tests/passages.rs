//! Exact reader navigation, clipboard requests and durable visible highlights.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus, InputKind, Mode},
    model::IncomingArticle,
    reader,
    store::Store,
};
use ratatui::style::Color;

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn keys(app: &mut App, text: &str) -> Result<()> {
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    Ok(())
}

fn reading(
    body: &str,
    width: usize,
    height: usize,
) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    store.ingest(&feed, "Selection journal", &[article(body)])?;
    let mut app = App::new(store, false)?;
    app.resize_reader(width, height);
    press(&mut app, KeyCode::Enter)?;
    Ok((directory, app))
}

fn article(body: &str) -> IncomingArticle {
    IncomingArticle {
        body_origin: lume::model::FeedOrigin::Feed,
        external_id: "selection".into(),
        title: "Precise passages".into(),
        url: "https://journal.example/selection".into(),
        author: "Original fixture".into(),
        published: 2_000_000_000,
        body: body.into(),
    }
}

#[test]
fn exact_yank_and_source_capture_survive_soft_wrapping_and_reflow() -> Result<()>
{
    let body = "Alpha **café** &amp; [dados](https://example.org).";
    let (_directory, mut app) = reading(body, 12, 3)?;
    keys(&mut app, "wvlll")?;
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("café".into())
    );
    app.resize_reader(8, 4);
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("café".into())
    );
    keys(&mut app, "n")?;
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Expected an annotation draft");
    };
    let InputKind::Highlight { passage, .. } = &input.kind else {
        anyhow::bail!("Expected a captured quotation");
    };
    assert_eq!(passage.quote, "café");
    assert_eq!(
        passage.anchor,
        body.find("café").context("Fixture lost its quote")?
    );
    Ok(())
}

#[test]
fn forward_and_backward_yanks_do_not_include_soft_wrap_newlines() -> Result<()>
{
    let (_directory, mut app) = reading("alpha beta gamma delta", 7, 3)?;
    keys(&mut app, "vwlll")?;
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("alpha beta".into())
    );
    press(&mut app, KeyCode::Esc)?;
    keys(&mut app, "vb")?;
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("beta".into())
    );
    Ok(())
}

#[test]
fn graphemes_and_vertical_goal_preserve_combining_marks_and_wide_cells()
-> Result<()> {
    let (_directory, mut app) = reading("a e\u{301} 界 👩‍💻 z", 40, 3)?;
    keys(&mut app, "llv")?;
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("e\u{301}".into())
    );
    keys(&mut app, "llll")?;
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("e\u{301} 界 👩‍💻".into())
    );
    let (_directory, mut app) = reading("```\nabcdef\nx\nabcdef\n```", 20, 2)?;
    keys(&mut app, "lllj")?;
    assert_eq!(app.reader_column, 0);
    keys(&mut app, "j")?;
    assert_eq!(app.reader_column, 3);
    assert_eq!(app.scroll, 1);
    Ok(())
}

#[test]
fn linewise_selection_preserves_code_and_tab_changes_focus() -> Result<()> {
    let (_directory, mut app) =
        reading("```\n  let n = 42;  \nnext();\n```", 12, 3)?;
    keys(&mut app, "Vj")?;
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("  let n = 42;  \nnext();".into())
    );
    press(&mut app, KeyCode::Esc)?;
    keys(&mut app, "0$")?;
    assert_eq!(app.reader_column, 6);
    press(&mut app, KeyCode::BackTab)?;
    assert_eq!(app.focus, Focus::Articles);
    Ok(())
}

fn marked_bytes(app: &App) -> Vec<usize> {
    let mut marked = Vec::new();
    for (line, range) in app.reader_styled.iter().zip(&app.reader_ranges) {
        let mut offset = range.start;
        for span in &line.spans {
            if span.style.bg == Some(Color::Rgb(65, 53, 32)) {
                marked.extend(offset..offset + span.content.len());
            }
            offset += span.content.len();
        }
    }
    marked
}

#[test]
fn saved_marks_keep_exact_duplicate_identity_and_support_notes_and_undo()
-> Result<()> {
    let body = "first target and second target";
    let (directory, mut app) = reading(body, 40, 5)?;
    keys(&mut app, "wwwwvllllls")?;
    assert_eq!(app.comments[0].quote, "target");
    assert_eq!(app.comments[0].anchor, 24);
    assert_eq!(marked_bytes(&app), (24..30).collect::<Vec<_>>());
    keys(&mut app, ":edit-note")?;
    press(&mut app, KeyCode::Enter)?;
    keys(&mut app, "A precise note")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    app.save_session()?;
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    assert_eq!(app.comments[0].body, "A precise note");
    assert_eq!(marked_bytes(&app), (24..30).collect::<Vec<_>>());
    keys(&mut app, "uu")?;
    assert!(app.comments.is_empty());
    assert!(marked_bytes(&app).is_empty());
    Ok(())
}

#[test]
fn publisher_changes_relocate_unique_quotes_and_leave_ambiguous_quotes_unmarked()
-> Result<()> {
    let (directory, mut app) =
        reading("first target and second target", 40, 5)?;
    keys(&mut app, "wwwwvllllls")?;
    let mut writer = Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = writer.feeds()?.remove(0);
    writer.ingest(
        &feed,
        "Updated",
        &[article("prefix first target and second target")],
    )?;
    app.reload()?;
    assert!(marked_bytes(&app).is_empty());
    assert_eq!(app.comments[0].quote, "target");
    writer.ingest(&feed, "Updated", &[article("Only target remains")])?;
    app.reload()?;
    assert_eq!(marked_bytes(&app), (5..11).collect::<Vec<_>>());
    Ok(())
}

#[test]
fn oversized_and_empty_selections_fail_without_saving_or_losing_the_selection()
-> Result<()> {
    let (_directory, mut app) =
        reading(&format!("```\n{}\n```", "x".repeat(65_537)), 40, 3)?;
    keys(&mut app, "v$")?;
    assert!(press(&mut app, KeyCode::Char('y')).is_err());
    assert!(press(&mut app, KeyCode::Char('s')).is_err());
    assert!(app.selection.is_some());
    assert!(app.comments.is_empty());
    let directory = tempfile::tempdir()?;
    let mut empty =
        App::new(Store::open(&directory.path().join("empty.db"))?, false)?;
    keys(&mut empty, "V")?;
    assert!(press(&mut empty, KeyCode::Char('y')).is_err());
    Ok(())
}

#[test]
fn source_projection_handles_entities_inline_code_links_and_emphasis()
-> Result<()> {
    for (body, visible, expected) in [
        ("`abcdef`", "bc", "bc"),
        ("A &amp; B", "&", "&amp;"),
        ("**café**", "fé", "fé"),
        ("[dados](https://example.org)", "ados", "ados"),
        ("    first();\n    second();", "second", "second"),
        ("````\nα();\nβ();\n````", "β", "β"),
    ] {
        let rendered = reader::render(body, 40);
        let start = rendered
            .text
            .find(visible)
            .context("Missing visible text")?;
        let range =
            reader::to_source(&rendered.mappings, start..start + visible.len())
                .context("Missing source range")?;
        assert_eq!(&body[range], expected, "{body}");
    }
    Ok(())
}

#[test]
fn overlapping_marks_form_a_visible_union_and_undo_keeps_the_older_mark()
-> Result<()> {
    let (_directory, mut app) = reading("alpha beta gamma", 40, 5)?;
    keys(&mut app, "vlllls")?;
    keys(&mut app, "0lllvlllllls")?;
    assert_eq!(marked_bytes(&app), (0..10).collect::<Vec<_>>());
    keys(&mut app, "u")?;
    assert_eq!(marked_bytes(&app), (0..5).collect::<Vec<_>>());
    Ok(())
}

#[test]
fn repeating_a_visual_action_does_not_toggle_the_queue_or_save_again()
-> Result<()> {
    use crossterm::event::KeyEventKind;
    let (_directory, mut app) = reading("alpha beta gamma", 40, 5)?;
    keys(&mut app, "vlllls")?;
    for character in ['s', 'v', 'V', 'y'] {
        for _ in 0..20 {
            let action = app.key(KeyEvent::new_with_kind(
                KeyCode::Char(character),
                KeyModifiers::NONE,
                KeyEventKind::Repeat,
            ))?;
            assert_eq!(action, Action::None);
        }
    }
    assert_eq!(app.comments.len(), 1);
    assert!(!app.current().context("Missing article")?.queued);
    assert!(app.selection.is_none());
    Ok(())
}

#[test]
fn reflow_preserves_a_cursor_and_selection_on_an_empty_display_row()
-> Result<()> {
    let (_directory, mut app) = reading("alpha\n\nbeta gamma delta", 12, 4)?;
    keys(&mut app, "jv")?;
    assert_eq!(app.reader_cursor, 1);
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("\n".into())
    );
    app.resize_reader(40, 4);
    assert_eq!(app.reader_cursor, 1);
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("\n".into())
    );
    Ok(())
}

#[test]
fn cursor_and_selected_cells_render_inside_small_and_large_reader_viewports()
-> Result<()> {
    use lume::ui;
    use ratatui::{Terminal, backend::TestBackend, layout::Rect};
    for (width, height) in [(24, 10), (52, 18), (100, 30), (144, 44)] {
        let (_directory, mut app) = reading("a café 界 🦀", 40, 5)?;
        keys(&mut app, "Z")?;
        let content = ui::workspace_reader_content(
            Rect::new(0, 0, width, height),
            &app.workspace,
        );
        app.resize_reader(content.width.into(), content.height.into());
        keys(&mut app, "llvlll")?;
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        let buffer = terminal.backend().buffer();
        assert_eq!(buffer[(content.x + 2, content.y)].symbol(), "c");
        assert_eq!(
            buffer[(content.x + 2, content.y)].bg,
            Color::Rgb(225, 179, 109)
        );
        assert_eq!(buffer[(content.x, content.y)].bg, Color::Rgb(23, 23, 26));
    }
    Ok(())
}
