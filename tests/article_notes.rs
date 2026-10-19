//! Passage markers, precise note mutations and nonmodal pointer previews.

use anyhow::{Context, Result};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseButton,
    MouseEvent, MouseEventKind,
};
use lume::{
    app::{
        Action, AnnotationPosition, App, Focus, InputBatch, InputKind, Mode,
    },
    model::{FeedOrigin, IncomingArticle},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 140, 36);
const BODY: &str = concat!(
    "# Systems\n\n",
    "A café 🦀 explanation about consensus and data.\n\n",
    "A later paragraph about failover.\n",
);

fn article() -> IncomingArticle {
    IncomingArticle {
        body_origin: FeedOrigin::Feed,
        external_id: "systems".into(),
        title: "Systems and notes".into(),
        url: "https://journal.example/systems".into(),
        author: "An author".into(),
        published: 1_700_000_000,
        body: BODY.into(),
    }
}

fn application(notes: &[(&str, &str)]) -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = store.subscribe("https://journal.example/rss")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|item| item.id == feed)
        .context("Feed missing")?;
    store.ingest(&feed, "Systems", &[article()])?;
    let article = store.library()?.articles[0].id;
    for (quote, body) in notes {
        if quote.is_empty() {
            store.save_comment(article, None, body)?;
        } else {
            let anchor = BODY.find(quote).context("Quote missing")?;
            store.save_highlight(article, quote, anchor, body)?;
        }
    }
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.workspace.fullscreen = true;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn press(app: &mut App, character: char) -> Result<Action> {
    app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<()> {
    press(app, ':')?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn cursor_on(app: &mut App, text: &str) -> Result<usize> {
    let row = app
        .reader_lines
        .iter()
        .position(|line| line.contains(text))
        .context("Reader row missing")?;
    app.reader_cursor = row;
    app.reader_column = app.reader_lines[row].find(text).unwrap_or(0);
    Ok(row)
}

fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    }
}

fn content(app: &App) -> Result<Rect> {
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader missing")?;
    Ok(ui::workspace_reader_content(area, &app.workspace))
}

#[test]
fn dd_removes_the_note_under_the_cursor_and_undo_restores_it() -> Result<()> {
    let (directory, mut app) = application(&[
        ("", "First private note"),
        ("", "Second private note"),
    ])?;
    let article = app.current().context("Article missing")?.id;
    let removed = app.comments[1].id;
    cursor_on(&mut app, "Second private note")?;
    app.comment_selected = 0;

    press(&mut app, 'd')?;
    assert_eq!(app.comments.len(), 2);
    press(&mut app, 'd')?;

    assert_eq!(app.comments.len(), 1);
    assert_eq!(app.comments[0].body, "First private note");
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(!store.is_deleted(article)?);
    assert_eq!(store.body(article)?, BODY);
    press(&mut app, 'u')?;
    assert!(app.comments.iter().any(|note| note.id == removed));
    Ok(())
}

#[test]
fn note_deletion_cancels_on_escape_motion_and_focus_loss() -> Result<()> {
    let (_directory, mut app) = application(&[("", "Private note")])?;
    cursor_on(&mut app, "Private note")?;
    press(&mut app, 'd')?;
    app.key(KeyEvent::new_with_kind(
        KeyCode::Char('d'),
        KeyModifiers::NONE,
        KeyEventKind::Repeat,
    ))?;
    assert_eq!(app.comments.len(), 1);
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert_eq!(app.focus, Focus::Reader);
    press(&mut app, 'd')?;
    let mut batch = InputBatch::new(vec![Event::FocusLost]);
    batch.dispatch_next(&mut app)?;
    press(&mut app, 'd')?;
    assert_eq!(app.comments.len(), 1);
    press(&mut app, 'h')?;
    press(&mut app, 'd')?;
    assert_eq!(app.comments.len(), 1);
    Ok(())
}

#[test]
fn annotations_pane_accepts_dd_and_preserves_publisher_text() -> Result<()> {
    let (_directory, mut app) = application(&[("café", "A passage note")])?;
    app.workspace.fullscreen = false;
    command(&mut app, "annotations")?;
    assert_eq!(app.focus, Focus::Annotations);

    press(&mut app, 'd')?;
    press(&mut app, 'd')?;

    assert!(app.annotations.is_empty());
    assert_eq!(app.body, BODY);
    press(&mut app, 'u')?;
    assert_eq!(app.comments[0].quote, "café");
    Ok(())
}

#[test]
fn find_prefix_and_visual_selection_keep_their_vim_semantics() -> Result<()> {
    let (_directory, mut app) = application(&[("", "Second private note")])?;
    cursor_on(&mut app, "Second private note")?;
    press(&mut app, 'f')?;
    press(&mut app, 'd')?;
    assert_eq!(app.comments.len(), 1);
    assert_eq!(app.reader_column, 5);
    press(&mut app, 'v')?;
    press(&mut app, 'd')?;
    press(&mut app, 'd')?;
    assert_eq!(app.comments.len(), 1);
    Ok(())
}

#[test]
fn new_cursor_note_captures_source_and_gets_a_marker() -> Result<()> {
    let (_directory, mut app) = application(&[])?;
    cursor_on(&mut app, "café")?;

    press(&mut app, 'n')?;

    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Expected a note editor");
    };
    let InputKind::Highlight { passage, .. } = &input.kind else {
        anyhow::bail!("Expected a captured passage");
    };
    assert!(passage.quote.contains("café 🦀"));
    app.paste("Consensus deserves study")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    let row = cursor_on(&mut app, "café")?;
    assert_eq!(app.reader_note_markers(row), [app.comments[0].id]);
    assert_eq!(app.body, BODY);
    assert!(matches!(
        app.annotations[0].position,
        AnnotationPosition::Located { .. }
    ));
    Ok(())
}

#[test]
fn search_n_stays_a_motion_and_annotate_can_override_it() -> Result<()> {
    let (_directory, mut app) = application(&[])?;
    command(&mut app, "find consensus")?;
    press(&mut app, 'n')?;
    assert!(matches!(app.mode, Mode::Normal));
    assert!(app.comments.is_empty());

    command(&mut app, "annotate")?;

    assert!(matches!(app.mode, Mode::Input(_)));
    Ok(())
}

#[test]
fn footer_edit_and_enter_target_the_actual_note_and_keep_its_quote()
-> Result<()> {
    let (_directory, mut app) = application(&[
        ("", "First private note"),
        ("café", "Second private note"),
    ])?;
    let id = app.comments[1].id;
    cursor_on(&mut app, "Second private note")?;
    app.comment_selected = 0;
    press(&mut app, 'e')?;
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Expected an editor");
    };
    assert!(matches!(input.kind,
        InputKind::Comment { comment: Some(target), .. } if target == id));
    app.paste(" revised")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert_eq!(app.comments[1].quote, "café");
    assert_eq!(app.comments[1].body, "Second private note revised");
    cursor_on(&mut app, "café")?;

    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;

    assert!(matches!(app.mode, Mode::Input(_)));
    Ok(())
}

#[test]
fn hover_batches_show_the_note_and_clicking_popup_edits_it() -> Result<()> {
    let (_directory, mut app) = application(&[("café", "Consensus insight")])?;
    let row = cursor_on(&mut app, "café")?;
    let column = content(&app)?;
    let x = column.x - 1;
    let y = column.y + row as u16;
    let mut batch = InputBatch::new(vec![
        Event::Mouse(mouse(MouseEventKind::Moved, 0, 0)),
        Event::Mouse(mouse(MouseEventKind::Moved, x, y)),
    ]);
    assert!(!batch.needs_redraw());
    batch.dispatch_next(&mut app)?;
    assert!(batch.is_empty());
    let (id, anchor) = app.hovered_note().context("Hover missing")?;
    assert_eq!(id, app.comments[0].id);
    let mut terminal = Terminal::new(TestBackend::new(140, 36))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let buffer = terminal.backend().buffer();
    let visible = buffer
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect::<String>();
    assert_eq!(buffer[(x, y)].symbol(), "●");
    assert!(visible.contains("Consensus insight"));
    let popup = ui::note_preview_area(SCREEN, anchor);
    app.mouse(mouse(MouseEventKind::Moved, popup.x + 1, popup.y + 1))?;
    assert_eq!(app.hovered_note(), Some((id, anchor)));

    app.mouse(mouse(
        MouseEventKind::Down(MouseButton::Left),
        popup.x + 1,
        popup.y + 1,
    ))?;

    assert!(matches!(app.mode, Mode::Input(_)));
    assert!(app.hovered_note().is_none());
    Ok(())
}

#[test]
fn hover_dismisses_off_target_and_reflow_rebuilds_marker_positions()
-> Result<()> {
    let (_directory, mut app) = application(&[("café", "Private insight")])?;
    let row = cursor_on(&mut app, "café")?;
    let column = content(&app)?;
    app.mouse(mouse(
        MouseEventKind::Moved,
        column.x - 1,
        column.y + row as u16,
    ))?;
    assert!(app.hovered_note().is_some());
    app.mouse(mouse(MouseEventKind::Moved, 0, 0))?;
    assert!(app.hovered_note().is_none());

    app.resize_reader(15, 20);

    let row = cursor_on(&mut app, "café")?;
    assert_eq!(app.reader_note_markers(row), [app.comments[0].id]);
    assert!(app.hovered_note().is_none());
    Ok(())
}

#[test]
fn general_notes_do_not_invent_a_passage_marker() -> Result<()> {
    let (_directory, mut app) = application(&[])?;
    command(&mut app, "note")?;
    app.paste("A general thought")?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert!(app.comments[0].quote.is_empty());
    assert!(
        app.annotations
            .iter()
            .all(|entry| entry.position == AnnotationPosition::General)
    );
    assert!(
        (0..app.reader_lines.len())
            .all(|row| app.reader_note_markers(row).is_empty())
    );
    Ok(())
}

#[test]
fn hover_works_over_precise_unicode_text_and_disabled_mouse_dismisses_it()
-> Result<()> {
    let (_directory, mut app) = application(&[("café 🦀", "Unicode insight")])?;
    let row = cursor_on(&mut app, "café")?;
    let column = content(&app)?;
    let event =
        mouse(MouseEventKind::Moved, column.x + 2, column.y + row as u16);
    app.mouse(event)?;
    assert_eq!(
        app.hovered_note().map(|(id, _)| id),
        Some(app.comments[0].id)
    );
    app.mouse(MouseEvent {
        kind: MouseEventKind::ScrollRight,
        ..event
    })?;
    assert!(app.hovered_note().is_none());
    app.mouse(event)?;

    command(&mut app, "mouse off")?;
    app.mouse(event)?;

    assert!(app.hovered_note().is_none());
    assert_eq!(app.focus, Focus::Reader);
    Ok(())
}

#[test]
fn overlapping_notes_share_a_marker_and_stale_delete_prefix_is_canceled()
-> Result<()> {
    let (_directory, mut app) = application(&[
        ("café", "First insight"),
        ("café 🦀", "Second insight"),
    ])?;
    let row = cursor_on(&mut app, "café")?;
    assert_eq!(app.reader_note_markers(row).len(), 2);
    press(&mut app, 'd')?;

    app.reload()?;
    press(&mut app, 'd')?;

    assert_eq!(app.comments.len(), 2);
    press(&mut app, 'd')?;
    assert_eq!(app.comments[0].body, "Second insight");
    Ok(())
}

#[test]
fn ambiguous_updated_quotes_keep_their_note_without_a_false_marker()
-> Result<()> {
    let (directory, app) = application(&[("café", "Keep my insight")])?;
    drop(app);
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = store.feeds()?.remove(0);
    let mut updated = article();
    updated.body = "Changed introduction.\n\ncafé and café.\n".into();
    store.ingest(&feed, "Systems", &[updated])?;

    let app = App::new(store, false)?;

    assert_eq!(app.annotations[0].position, AnnotationPosition::Unplaced);
    assert!(
        (0..app.reader_lines.len())
            .all(|row| app.reader_note_markers(row).is_empty())
    );
    assert_eq!(app.comments[0].body, "Keep my insight");
    Ok(())
}

#[test]
fn annotation_previews_stay_inside_small_viewports_in_all_themes() -> Result<()>
{
    let (_directory, mut app) = application(&[("café", "Private insight")])?;
    for theme in ["lume", "nord", "gruvbox"] {
        app.workspace.theme = theme.into();
        for (width, height) in [(24, 10), (40, 18), (80, 24), (140, 36)] {
            let screen = Rect::new(0, 0, width, height);
            app.resize_workspace(screen);
            let row = cursor_on(&mut app, "café")?;
            app.scroll = row;
            let column = ui::workspace_reader_content(screen, &app.workspace);
            app.mouse(mouse(MouseEventKind::Moved, column.x - 1, column.y))?;
            let (_, anchor) = app.hovered_note().context("No preview")?;
            let popup = ui::note_preview_area(screen, anchor);
            assert!(screen.contains(popup.as_position()));
            assert!(popup.right() <= width);
            assert!(popup.bottom() <= height);
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            terminal.draw(|frame| ui::draw(frame, &app))?;
        }
    }
    Ok(())
}

#[test]
fn a_stale_cursor_reports_an_error_without_writing_a_note() -> Result<()> {
    let (_directory, mut app) = application(&[])?;
    app.reader_cursor = usize::MAX;
    press(&mut app, ':')?;
    app.paste("annotate")?;

    let result = app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(result.is_err());
    assert!(matches!(app.mode, Mode::Input(_)));
    assert!(app.comments.is_empty());
    Ok(())
}

#[test]
fn jumping_to_an_overlapping_note_keeps_it_as_the_deletion_target() -> Result<()>
{
    let (_directory, mut app) = application(&[
        ("café", "First insight"),
        ("café 🦀", "Chosen insight"),
    ])?;
    let chosen = app.comments[1].id;
    app.workspace.fullscreen = false;
    command(&mut app, "annotations")?;
    app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_eq!(app.cursor_note(), Some(chosen));

    press(&mut app, 'd')?;
    press(&mut app, 'd')?;

    assert_eq!(app.comments.len(), 1);
    assert_eq!(app.comments[0].body, "First insight");
    Ok(())
}
