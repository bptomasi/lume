//! Pane-local mouse gestures and keyboard equivalents through real app state.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent,
    MouseEventKind,
};
use lume::{
    app::{Action, AnnotationPosition, App, Focus, InputBatch},
    model::{Pane, Workspace},
    source,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 160, 44);
const BODY: &str = "# Operations\n\nA café e\u{301} 界 🦀 passage.\n\nLater recovery passage.\n\n```rust\n\tlet value = 42;\n```\n";

fn application() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = BODY.into();
    store.ingest(&feed, "Journal", &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<()> {
    press(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    press(app, KeyCode::Enter)?;
    Ok(())
}

fn mouse(
    app: &mut App,
    kind: MouseEventKind,
    column: u16,
    row: u16,
) -> Result<()> {
    app.mouse(MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}

fn click(app: &mut App, column: u16, row: u16) -> Result<()> {
    mouse(app, MouseEventKind::Down(MouseButton::Left), column, row)?;
    mouse(app, MouseEventKind::Up(MouseButton::Left), column, row)
}

fn reader_content(app: &App) -> Result<Rect> {
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    Ok(ui::workspace_reader_content(area, &app.workspace))
}

fn reader_row(app: &App, needle: &str) -> Result<u16> {
    let row = app
        .reader_lines
        .iter()
        .position(|line| line.contains(needle))
        .context("Missing fixture row")?;
    Ok(reader_content(app)?.y + row as u16 - app.scroll as u16)
}

#[test]
fn mouse_selection_maps_wide_combining_and_emoji_cells_to_exact_source()
-> Result<()> {
    let (directory, mut app) = application()?;
    let column = reader_content(&app)?.x;
    let row = reader_row(&app, "café")?;

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        column + 2,
        row,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        column + 13,
        row,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        column + 13,
        row,
    )?;

    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("café e\u{301} 界 🦀".into())
    );
    press(&mut app, KeyCode::Char('s'))?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let note = store
        .comments(app.current().context("Missing article")?.id)?
        .remove(0);
    assert_eq!(note.quote, "café e\u{301} 界 🦀");
    assert_eq!(note.anchor, BODY.find("café").context("Missing source")?);
    Ok(())
}

#[test]
fn dragging_across_a_sidebar_stays_in_the_original_reader_column() -> Result<()>
{
    let (_directory, mut app) = application()?;
    let column = reader_content(&app)?.x;
    let row = reader_row(&app, "café")?;

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        column + 2,
        row,
    )?;
    mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 1, row)?;
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), 1, row)?;

    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("A c".into())
    );
    assert_eq!(app.focus, Focus::Reader);
    Ok(())
}

#[test]
fn horizontally_scrolled_code_and_tabs_keep_their_mouse_cell_mapping()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let column = reader_content(&app)?.x;
    let row = reader_row(&app, "let value")?;
    for _ in 0..4 {
        mouse(&mut app, MouseEventKind::ScrollRight, column + 4, row)?;
    }

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        column + 4,
        row,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        column + 8,
        row,
    )?;

    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("value".into())
    );
    Ok(())
}

#[test]
fn divider_drag_previews_without_writing_then_commits_on_release() -> Result<()>
{
    let (directory, mut app) = application()?;
    app.save_session()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let initial = store.workspace()?;
    let boundary = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .articles
        .context("Articles hidden")?
        .x;

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        boundary,
        12,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        boundary + 5,
        12,
    )?;

    assert_eq!(app.workspace.library_width, initial.library_width + 5);
    assert_eq!(app.workspace.article_width, initial.article_width - 5);
    assert_eq!(store.workspace()?.library_width, initial.library_width);
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        boundary + 5,
        12,
    )?;
    assert_eq!(store.workspace()?.library_width, initial.library_width + 5);
    assert_eq!(store.workspace()?.article_width, initial.article_width - 5);
    Ok(())
}

#[test]
fn keys_focus_loss_and_resize_cancel_uncommitted_divider_widths() -> Result<()>
{
    let (_directory, mut app) = application()?;
    let initial = app.workspace.library_width;
    for interruption in [
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        Event::FocusLost,
        Event::Resize(144, 40),
        Event::Key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
    ] {
        app.resize_workspace(SCREEN);
        let boundary = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
            .articles
            .context("Articles hidden")?
            .x;
        mouse(
            &mut app,
            MouseEventKind::Down(MouseButton::Left),
            boundary,
            12,
        )?;
        mouse(
            &mut app,
            MouseEventKind::Drag(MouseButton::Left),
            boundary + 4,
            12,
        )?;
        let mut batch = InputBatch::new(vec![interruption]);
        batch.dispatch_next(&mut app)?;
        assert_eq!(app.workspace.library_width, initial);
    }
    Ok(())
}

#[test]
fn header_drag_and_keyboard_move_persist_the_same_pane_order() -> Result<()> {
    let (directory, mut app) = application()?;
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let library = areas.library.context("Library hidden")?;
    let reader = areas.reader.context("Reader hidden")?;

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        library.x + 4,
        library.y,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        reader.x + 4,
        reader.y,
    )?;

    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Articles,
            Pane::Reader,
            Pane::Library,
            Pane::Annotations
        ]
    );
    command(&mut app, "panel-focus library")?;
    command(&mut app, "panel-move left")?;
    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Articles,
            Pane::Library,
            Pane::Reader,
            Pane::Annotations
        ]
    );
    let stored = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(stored.workspace()?.panel_order, app.workspace.panel_order);
    Ok(())
}

#[test]
fn every_pane_can_collapse_and_the_small_restore_buttons_remain_reachable()
-> Result<()> {
    let (_directory, mut app) = application()?;
    command(&mut app, "annotations")?;
    for pane in Pane::ALL {
        command(&mut app, &format!("panel-focus {}", pane.name()))?;
        let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
            .get(pane)
            .context("Pane hidden")?;
        let button =
            ui::pane_close_rect(area).context("Close control absent")?;
        click(&mut app, button.x + 1, button.y)?;
        assert!(!app.workspace.pane_visible(pane));
    }
    assert!(
        ui::workspace_areas(SCREEN, app.focus, &app.workspace)
            .ordered()
            .is_empty()
    );
    let screen = Rect::new(0, 0, 24, 10);
    app.resize_workspace(screen);
    let (pane, button) = ui::pane_buttons(screen, &app.workspace).remove(2);
    click(&mut app, button.x + 2, button.y)?;
    assert_eq!(pane, Pane::Reader);
    assert!(app.workspace.show_reader);
    Ok(())
}

#[test]
fn ctrl_w_keeps_layout_keys_out_of_reader_motions_and_reopens_notes()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    press(&mut app, KeyCode::Char('a'))?;
    assert_eq!(app.focus, Focus::Annotations);
    assert!(app.workspace.show_annotations);
    let original = app.workspace.annotation_width;
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    press(&mut app, KeyCode::Char('+'))?;
    assert_eq!(app.workspace.annotation_width, original + 2);
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    press(&mut app, KeyCode::Char('c'))?;
    assert!(!app.workspace.show_annotations);
    assert_eq!(app.focus, Focus::Reader);
    Ok(())
}

#[test]
fn annotations_sort_by_source_position_and_general_notes_follow_passages()
-> Result<()> {
    let (directory, app) = application()?;
    let id = app.current().context("Article missing")?.id;
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_highlight(
        id,
        "Later recovery",
        BODY.find("Later recovery").context("Anchor missing")?,
        "Later insight",
    )?;
    store.save_highlight(
        id,
        "café",
        BODY.find("café").context("Anchor missing")?,
        "First insight",
    )?;
    store.save_comment(id, None, "General thought")?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);

    command(&mut app, "annotations")?;

    let ordered = app
        .annotations
        .iter()
        .map(|entry| app.comments[entry.comment_index].body.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ordered,
        ["First insight", "Later insight", "General thought"]
    );
    assert_eq!(app.annotation_selected(), Some(0));
    press(&mut app, KeyCode::Down)?;
    press(&mut app, KeyCode::Enter)?;
    assert_eq!(app.focus, Focus::Reader);
    assert!(app.reader_lines[app.reader_cursor].contains("Later recovery"));
    assert!(app.workspace.show_annotations);
    Ok(())
}

#[test]
fn legacy_layout_defaults_are_compatible_and_duplicate_order_is_rejected()
-> Result<()> {
    let legacy: Workspace = serde_json::from_str(r#"{"show_library":false}"#)?;
    assert_eq!(legacy.panel_order, Pane::ALL);
    assert!(legacy.show_reader);
    assert!(!legacy.show_annotations);
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut invalid = legacy;
    invalid.panel_order = [Pane::Reader; 4];
    assert!(store.save_workspace(&invalid).is_err());
    assert_eq!(store.workspace()?.panel_order, Pane::ALL);
    Ok(())
}

#[test]
fn hover_retains_input_without_forcing_frames_and_disabled_mouse_keeps_focus()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let batch = InputBatch::new(vec![Event::Mouse(MouseEvent {
        kind: MouseEventKind::Moved,
        column: 5,
        row: 12,
        modifiers: KeyModifiers::NONE,
    })]);
    assert!(!batch.is_empty());
    assert!(!batch.needs_redraw());
    command(&mut app, "mouse off")?;
    let before = (app.focus, app.current().context("Article missing")?.id);
    click(&mut app, 5, 12)?;
    assert_eq!(
        (app.focus, app.current().context("Article missing")?.id),
        before
    );
    Ok(())
}

#[test]
fn resized_and_reordered_layouts_render_all_four_panes_without_overflow()
-> Result<()> {
    let (_directory, mut app) = application()?;
    command(&mut app, "annotations")?;
    app.workspace.library_width = 48;
    app.workspace.article_width = 80;
    app.workspace.annotation_width = 80;
    for (width, height) in [(160, 44), (120, 24), (100, 20), (40, 12), (24, 10)]
    {
        let screen = Rect::new(0, 0, width, height);
        app.resize_workspace(screen);
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        terminal.draw(|frame| ui::draw(frame, &app))?;
        let areas = ui::workspace_areas(screen, app.focus, &app.workspace);
        for (_, area) in areas.ordered() {
            assert!(area.width >= 16 || width < 80);
            assert!(area.right() <= width);
        }
        assert!(areas.annotations.is_some());
    }
    assert!(app.annotations.is_empty());
    Ok(())
}

#[test]
fn ambiguous_refreshed_quotes_remain_unplaced_and_cannot_jump_to_a_guess()
-> Result<()> {
    let (directory, app) = application()?;
    let id = app.current().context("Article missing")?.id;
    drop(app);
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_highlight(
        id,
        "Later recovery",
        BODY.find("Later recovery").context("Anchor missing")?,
        "Original insight",
    )?;
    let feed = store.feeds()?.remove(0);
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = format!(
        "{}Later recovery and Later recovery.",
        "New padding ".repeat(20)
    );
    store.ingest(&feed, "Journal", &parsed.articles)?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);

    command(&mut app, "annotations")?;

    assert_eq!(app.annotations[0].position, AnnotationPosition::Unplaced);
    let error = press(&mut app, KeyCode::Enter)
        .err()
        .context("Ambiguous jump succeeded")?;
    assert!(error.to_string().contains("ambiguous"));
    assert_eq!(app.comments[0].quote, "Later recovery");
    Ok(())
}

#[test]
fn notes_from_the_other_source_are_labeled_and_mouse_jumps_validate_that_source()
-> Result<()> {
    use lume::model::ContentSource;
    let (_directory, mut app) = cross_source_application()?;
    app.resize_workspace(SCREEN);
    command(&mut app, "annotations")?;
    assert!(matches!(
        app.annotations[0].position,
        AnnotationPosition::Located { .. }
    ));
    assert!(matches!(
        app.annotations[1].position,
        AnnotationPosition::OtherSource { .. }
    ));
    let pane = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .annotations
        .context("Notes hidden")?;

    click(&mut app, pane.x + 3, pane.y + 8)?;

    assert_eq!(app.content_source, ContentSource::Feed);
    assert!(app.reader_lines[app.reader_cursor].contains("café"));
    assert_eq!(app.comments.len(), 2);
    Ok(())
}

#[test]
fn deleting_a_focused_annotation_never_deletes_its_article_and_undo_restores_it()
-> Result<()> {
    let (directory, app) = application()?;
    let id = app.current().context("Article missing")?.id;
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_comment(id, None, "A recoverable note")?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    command(&mut app, "annotations")?;

    press(&mut app, KeyCode::Char('D'))?;

    assert!(app.annotations.is_empty());
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(!store.is_deleted(id)?);
    press(&mut app, KeyCode::Char('u'))?;
    assert_eq!(app.comments[0].body, "A recoverable note");
    assert_eq!(app.annotations.len(), 1);
    Ok(())
}

#[test]
fn wheel_bursts_coalesce_without_dropping_the_next_command() -> Result<()> {
    let (_directory, mut app) = application()?;
    command(&mut app, "help layout")?;
    let event = Event::Mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: 40,
        row: 15,
        modifiers: KeyModifiers::NONE,
    });
    let mut events = vec![event; 20];
    events.push(Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)));
    let mut batch = InputBatch::new(events);

    assert_eq!(batch.dispatch_next(&mut app)?, Some(Action::None));
    assert_eq!(app.help_scroll, 20);
    assert_eq!(batch.dispatch_next(&mut app)?, Some(Action::None));
    assert!(matches!(app.mode, lume::app::Mode::Normal));
    assert!(batch.is_empty());
    Ok(())
}

#[test]
fn mouse_can_activate_a_filtered_palette_choice_without_clicking_through()
-> Result<()> {
    let (_directory, mut app) = application()?;
    command(&mut app, "palette")?;
    app.paste("Article annotations source order")?;
    let (choices, _) = ui::picker_regions(SCREEN);

    click(&mut app, choices.x + 2, choices.y)?;

    assert!(app.workspace.show_annotations);
    assert_eq!(app.focus, Focus::Annotations);
    assert!(matches!(app.mode, lume::app::Mode::Normal));
    assert!(app.selection.is_none());
    Ok(())
}

fn cross_source_application() -> Result<(tempfile::TempDir, App)> {
    use lume::model::{
        ArticleTarget, ContentSource, ExtractedDocument, Passage,
    };
    let (directory, app) = application()?;
    let article = app.current().context("Article missing")?.clone();
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_highlight(
        article.id,
        "café",
        BODY.find("café").context("Anchor missing")?,
        "Feed insight",
    )?;
    let body = "Full source paragraph with an independent cache.";
    store.cache_document(
        &ArticleTarget {
            article: article.id,
            url: article.url,
        },
        &ExtractedDocument {
            final_url: "https://journal.example/full".into(),
            body: body.into(),
        },
    )?;
    store.save_passage(
        article.id,
        &Passage {
            quote: "Full source".into(),
            anchor: 0,
            source: ContentSource::Full,
            revision: 1,
        },
        "Full insight",
    )?;
    Ok((directory, App::new(store, false)?))
}

#[test]
fn resizing_with_wrapped_private_notes_preserves_publisher_selection()
-> Result<()> {
    let (directory, mut app) = application()?;
    let id = app.current().context("Missing article")?.id;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_comment(
        id,
        None,
        &"A thoughtful observation about recovery. ".repeat(8),
    )?;
    command(&mut app, "content feed")?;
    command(&mut app, "find café")?;
    press(&mut app, KeyCode::Char('v'))?;
    press(&mut app, KeyCode::Char('e'))?;
    let expected = press(&mut app, KeyCode::Char('y'))?;

    app.resize_reader(24, 20);

    assert!(app.selection.is_some(), "Reflow discarded the selection");
    assert_eq!(press(&mut app, KeyCode::Char('y'))?, expected);
    Ok(())
}

#[test]
fn reported_shift_press_does_not_consume_the_layout_prefix() -> Result<()> {
    use crossterm::event::ModifierKeyCode;
    let (_directory, mut app) = application()?;
    let mut batch = InputBatch::new(vec![
        Event::Key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL)),
        Event::Key(KeyEvent::new(
            KeyCode::Modifier(ModifierKeyCode::LeftShift),
            KeyModifiers::SHIFT,
        )),
        Event::Key(KeyEvent::new(KeyCode::Char('h'), KeyModifiers::SHIFT)),
    ]);

    while batch.dispatch_next(&mut app)?.is_some() {}

    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Library,
            Pane::Reader,
            Pane::Articles,
            Pane::Annotations,
        ]
    );
    Ok(())
}

#[test]
fn a_background_reload_does_not_discard_a_panel_drag() -> Result<()> {
    let (_directory, mut app) = application()?;
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let library = areas.library.context("Missing library")?;
    let reader = areas.reader.context("Missing reader")?;
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        library.x + 4,
        library.y,
    )?;

    app.reload()?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        reader.x + 4,
        reader.y,
    )?;

    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Articles,
            Pane::Reader,
            Pane::Library,
            Pane::Annotations,
        ]
    );
    Ok(())
}

#[test]
fn corner_grip_previews_the_target_and_persists_on_release_only() -> Result<()>
{
    let (directory, mut app) = application()?;
    let areas = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    let reader = areas.reader.context("Missing reader")?;
    let original = app.workspace.panel_order;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;

    begin_panel_drag(&mut app, SCREEN, Pane::Library)?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    let preview = app.panel_drag().context("Missing move preview")?;
    assert_eq!(preview.pane, Pane::Library);
    assert_eq!(preview.target, Some(Pane::Reader));
    assert_eq!(app.workspace.panel_order, original);
    assert_eq!(store.workspace()?.panel_order, original);
    let header = rendered_header(&app, reader)?;
    assert!(header.contains('│'), "Insertion seam was not drawn");
    assert!(
        !header.contains("Drop here"),
        "Insertion obscured the title"
    );

    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        reader.x + 4,
        12,
    )?;

    assert!(app.panel_drag().is_none());
    assert_eq!(
        store.workspace()?.panel_order,
        [
            Pane::Articles,
            Pane::Reader,
            Pane::Library,
            Pane::Annotations,
        ]
    );
    Ok(())
}

#[test]
fn responsive_layout_keeps_drag_targets_stable_when_an_inactive_header_is_grabbed()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let screen = Rect::new(0, 0, 100, 44);
    app.resize_workspace(screen);
    let areas = ui::workspace_areas(screen, app.focus, &app.workspace);
    let library = areas.library.context("Missing library")?;
    let reader = areas.reader.context("Missing reader")?;
    let grip = ui::pane_move_rect(library).context("Missing grip")?;

    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        grip.x,
        grip.y,
    )?;
    app.resize_workspace(screen);
    let during = ui::workspace_areas(screen, app.focus, &app.workspace);
    assert_eq!(during.reader, Some(reader));
    assert!(during.articles.is_none());
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        reader.x + 4,
        12,
    )?;

    let after = ui::workspace_areas(screen, app.focus, &app.workspace);
    assert!(
        after.reader.context("Missing reader")?.x
            < after.library.context("Missing library")?.x
    );
    Ok(())
}

#[test]
fn keyboard_reorders_visible_neighbors_instead_of_responsively_hidden_panes()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let screen = Rect::new(0, 0, 100, 44);
    app.resize_workspace(screen);

    app.key(KeyEvent::new(KeyCode::Left, KeyModifiers::ALT))?;

    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Reader,
            Pane::Library,
            Pane::Articles,
            Pane::Annotations,
        ]
    );
    let after = ui::workspace_areas(screen, app.focus, &app.workspace);
    assert_eq!(after.reader.context("Missing reader")?.x, 0);
    assert!(after.articles.is_none());
    Ok(())
}

#[test]
fn reader_and_notes_reorder_with_direct_and_prefixed_keys_and_survive_reopen()
-> Result<()> {
    let (directory, mut app) = application()?;
    command(&mut app, "sidebar")?;
    command(&mut app, "articles")?;
    command(&mut app, "annotations")?;
    command(&mut app, "panel-focus reader")?;

    app.key(KeyEvent::new(KeyCode::Right, KeyModifiers::ALT))?;
    let after = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    assert!(
        after.annotations.context("Missing annotations")?.x
            < after.reader.context("Missing reader")?.x
    );
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    assert_eq!(
        app.workspace.panel_order,
        [
            Pane::Library,
            Pane::Articles,
            Pane::Annotations,
            Pane::Reader,
        ]
    );
    app.key(KeyEvent::new(KeyCode::Char('w'), KeyModifiers::CONTROL))?;
    press(&mut app, KeyCode::Left)?;
    let restored = ui::workspace_areas(SCREEN, app.focus, &app.workspace);
    assert!(
        restored.reader.context("Missing reader")?.x
            < restored.annotations.context("Missing annotations")?.x
    );
    Ok(())
}

#[test]
fn keys_focus_loss_and_resize_cancel_panel_move_previews() -> Result<()> {
    let (directory, mut app) = application()?;
    let original = app.workspace.panel_order;
    for interruption in [
        Event::Key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
        Event::FocusLost,
        Event::Resize(150, 40),
    ] {
        app.resize_workspace(SCREEN);
        begin_panel_drag(&mut app, SCREEN, Pane::Reader)?;
        mouse(&mut app, MouseEventKind::Drag(MouseButton::Left), 4, 12)?;
        let mut batch = InputBatch::new(vec![interruption]);
        batch.dispatch_next(&mut app)?;
        assert!(app.panel_drag().is_none());
        assert_eq!(app.workspace.panel_order, original);
    }
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.workspace()?.panel_order, original);
    Ok(())
}

#[test]
fn releasing_a_panel_outside_a_drop_target_keeps_its_order() -> Result<()> {
    let (directory, mut app) = application()?;
    let original = app.workspace.panel_order;
    begin_panel_drag(&mut app, SCREEN, Pane::Reader)?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        4,
        SCREEN.height - 1,
    )?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.workspace()?.panel_order, original);
    Ok(())
}

fn begin_panel_drag(app: &mut App, screen: Rect, pane: Pane) -> Result<()> {
    let area = ui::workspace_areas(screen, app.focus, &app.workspace)
        .get(pane)
        .context("Missing drag source")?;
    let grip = ui::pane_move_rect(area).context("Missing corner grip")?;
    mouse(
        app,
        MouseEventKind::Down(MouseButton::Left),
        grip.x + 1,
        grip.y,
    )
}

fn rendered_header(app: &App, area: Rect) -> Result<String> {
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    Ok((area.x..area.right())
        .map(|x| terminal.backend().buffer()[(x, area.y)].symbol())
        .collect())
}

#[test]
fn panel_moves_can_target_collapsed_top_tabs_without_expanding_them()
-> Result<()> {
    let (directory, mut app) = application()?;
    let (_, tab) = ui::pane_buttons(SCREEN, &app.workspace)
        .into_iter()
        .find(|(pane, _)| *pane == Pane::Annotations)
        .context("Missing annotations tab")?;
    begin_panel_drag(&mut app, SCREEN, Pane::Reader)?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        tab.x + 2,
        tab.y,
    )?;
    assert_eq!(
        app.panel_drag().context("Missing preview")?.target,
        Some(Pane::Annotations)
    );

    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        tab.x + 2,
        tab.y,
    )?;

    assert!(!app.workspace.show_annotations);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(
        store.workspace()?.panel_order,
        [
            Pane::Library,
            Pane::Articles,
            Pane::Annotations,
            Pane::Reader,
        ]
    );
    Ok(())
}

fn motion_frame(app: &mut App) -> Result<ratatui::buffer::Buffer> {
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    terminal.draw(|frame| ui::draw_interactive(frame, app))?;
    Ok(terminal.backend().buffer().clone())
}

#[test]
fn every_dragged_pane_keeps_its_entire_rounded_outline() -> Result<()> {
    let (_directory, mut app) = application()?;
    command(&mut app, "annotations")?;
    for pane in Pane::ALL {
        for capture_before_motion in [true, false] {
            let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
                .get(pane)
                .context("Pane hidden")?;
            let grip = ui::pane_move_rect(area).context("Grip hidden")?;
            let column = if area.x >= 7 { grip.x - 6 } else { grip.x + 8 };
            begin_panel_drag(&mut app, SCREEN, pane)?;
            if capture_before_motion {
                motion_frame(&mut app)?;
            }

            mouse(
                &mut app,
                MouseEventKind::Drag(MouseButton::Left),
                column,
                0,
            )?;
            let buffer = motion_frame(&mut app)?;

            let picture = app.floating_pane().context("Picture missing")?;
            assert_rounded_outline(picture.cells, picture.cells.area);
            assert_rounded_outline(&buffer, picture.area);
            for column in [area.x, area.right() - 1] {
                // An insertion marker may cross the vacated slot; the old
                // rounded corners and vertical outline must not remain.
                assert!(matches!(
                    buffer[(column, area.bottom() - 1)].symbol(),
                    " " | "─"
                ));
            }
            press(&mut app, KeyCode::Esc)?;
        }
    }
    Ok(())
}

fn assert_rounded_outline(buffer: &ratatui::buffer::Buffer, area: Rect) {
    for (column, row, symbol) in [
        (area.x, area.y, "╭"),
        (area.right() - 1, area.y, "╮"),
        (area.x, area.bottom() - 1, "╰"),
        (area.right() - 1, area.bottom() - 1, "╯"),
    ] {
        assert_eq!(buffer[(column, row)].symbol(), symbol);
    }
}

#[test]
fn held_pane_follows_the_grab_offset_with_frozen_cells_and_no_layout_write()
-> Result<()> {
    let (directory, mut app) = application()?;
    let original = app.workspace.panel_order;
    let text = app.reader_text.clone();
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Missing reader")?;
    let grip = ui::pane_move_rect(area).context("Missing grip")?;
    begin_panel_drag(&mut app, SCREEN, Pane::Reader)?;
    let before = motion_frame(&mut app)?;

    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        grip.x + 1 - 7,
        12,
    )?;
    app.reload()?;
    let after = motion_frame(&mut app)?;

    let floating = app.floating_pane().context("Missing held picture")?;
    assert_eq!(floating.area.x, area.x - 7);
    assert_eq!(floating.area.y, 12);
    assert_eq!(floating.slot, area);
    let original_cell = &before[(area.x + 4, area.y + 1)];
    assert_eq!(floating.cells[(area.x + 4, area.y + 1)], *original_cell);
    let mut held_cell = original_cell.clone();
    held_cell.set_bg(ratatui::style::Color::Rgb(13, 13, 15));
    assert_eq!(after[(floating.area.x + 4, floating.area.y + 1)], held_cell);
    assert_eq!(app.workspace.panel_order, original);
    assert_eq!(app.reader_text, text);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.workspace()?.panel_order, original);
    Ok(())
}

#[test]
fn held_picture_clips_at_screen_edges_and_dim_backdrop_restores_on_cancel()
-> Result<()> {
    use ratatui::style::Color;
    let (_directory, mut app) = application()?;
    for (theme, background, dimmed) in [
        ("lume", Color::Rgb(23, 23, 26), Color::Rgb(13, 13, 15)),
        ("nord", Color::Rgb(52, 60, 75), Color::Rgb(31, 36, 45)),
        ("gruvbox", Color::Rgb(40, 40, 40), Color::Rgb(24, 24, 24)),
    ] {
        app.workspace.theme = theme.into();
        begin_panel_drag(&mut app, SCREEN, Pane::Library)?;
        let original = motion_frame(&mut app)?;
        for (column, row) in [(80, 12), (159, 43), (80, 0)] {
            mouse(
                &mut app,
                MouseEventKind::Drag(MouseButton::Left),
                column,
                row,
            )?;
            let buffer = motion_frame(&mut app)?;
            let picture =
                app.floating_pane().context("Missing held picture")?;
            assert_eq!(picture.area.y, row);
            assert_eq!(buffer[(0, 0)].bg, dimmed);
            let corner = &buffer[(picture.area.x, row)];
            assert_eq!(corner.symbol(), original[(0, 2)].symbol());
            assert_eq!(corner.fg, original[(0, 2)].fg);
            assert_eq!(corner.bg, dimmed);
            assert_drag_surface(&buffer, picture.area, dimmed);
            let origin = picture.cells.area;
            assert_eq!(picture.cells[(origin.x, origin.y)].bg, background);
        }
        press(&mut app, KeyCode::Esc)?;
        let restored = motion_frame(&mut app)?;
        assert!(app.floating_pane().is_none());
        assert_eq!(restored[(0, 0)].bg, background);
    }
    Ok(())
}

fn assert_drag_surface(
    buffer: &ratatui::buffer::Buffer,
    area: Rect,
    background: ratatui::style::Color,
) {
    // A transparent corner alone leaves a whole-cell staircase. The curve,
    // adjoining edges and body must share the same dimmed neutral surface.
    for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        let position = (area.x + dx, area.y + dy);
        if buffer.area.contains(position.into()) {
            assert_eq!(buffer[position].bg, background);
        }
    }
}

#[test]
fn vertical_only_motion_settles_back_without_changing_saved_order() -> Result<()>
{
    use std::time::{Duration, Instant};
    let (directory, mut app) = application()?;
    let original = app.workspace.panel_order;
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .library
        .context("Library hidden")?;
    let column = ui::pane_move_rect(area).context("Grip hidden")?.x + 1;
    begin_panel_drag(&mut app, SCREEN, Pane::Library)?;
    motion_frame(&mut app)?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        column,
        42,
    )?;
    mouse(&mut app, MouseEventKind::Up(MouseButton::Left), column, 43)?;
    let picture = app.floating_pane().context("Missing return animation")?;
    assert_eq!(picture.area.x, picture.slot.x);
    assert_eq!(picture.area.y, 43);
    assert_eq!(picture.slot.y, 2);
    let now = Instant::now();

    app.tick_pane_motion(now + Duration::from_millis(80));

    let picture = app.floating_pane().context("Return ended too early")?;
    assert!((2..43).contains(&picture.area.y));
    app.tick_pane_motion(now + Duration::from_secs(1));
    assert!(app.floating_pane().is_none());
    assert_eq!(app.workspace.panel_order, original);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.workspace()?.panel_order, original);
    Ok(())
}

fn settling_application() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut app) = application()?;
    let reader = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Missing reader")?;
    begin_panel_drag(&mut app, SCREEN, Pane::Library)?;
    motion_frame(&mut app)?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        reader.x + 4,
        12,
    )?;
    Ok((directory, app))
}

#[test]
fn release_commits_before_the_transition_and_a_late_tick_finishes_without_backlog()
-> Result<()> {
    use std::time::{Duration, Instant};
    let (directory, mut app) = settling_application()?;
    let now = Instant::now();
    assert!(app.pane_motion_active());
    let moving = app.floating_pane().context("Missing release picture")?;
    assert!(moving.area.x < moving.slot.x);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(
        store.workspace()?.panel_order,
        [
            Pane::Articles,
            Pane::Reader,
            Pane::Library,
            Pane::Annotations,
        ]
    );

    assert!(!app.tick_pane_motion(now - Duration::from_secs(1)));
    assert!(app.tick_pane_motion(now + Duration::from_secs(1)));

    assert!(!app.pane_motion_active());
    assert!(app.floating_pane().is_none());
    assert!(!app.tick_pane_motion(now + Duration::from_secs(2)));
    Ok(())
}

#[test]
fn input_and_resize_interrupt_motion_without_delaying_the_requested_action()
-> Result<()> {
    let (_directory, mut app) = settling_application()?;
    press(&mut app, KeyCode::Char('?'))?;
    assert!(matches!(app.mode, lume::app::Mode::Help));
    assert!(!app.pane_motion_active());
    let (_directory, mut app) = settling_application()?;
    app.resize_workspace(Rect::new(0, 0, 120, 32));
    assert!(!app.pane_motion_active());
    assert!(app.floating_pane().is_none());
    let (_directory, mut app) = settling_application()?;
    let reader = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .reader
        .context("Reader hidden")?;
    mouse(
        &mut app,
        MouseEventKind::ScrollDown,
        reader.x + 4,
        reader.y + 8,
    )?;
    assert!(!app.pane_motion_active());
    Ok(())
}

#[test]
fn compact_header_uses_the_removed_breadcrumb_row_for_pane_controls()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let buffer = motion_frame(&mut app)?;
    let controls = (0..SCREEN.width)
        .map(|x| buffer[(x, 1)].symbol())
        .collect::<String>();
    assert!(controls.contains("Library"));
    assert!(controls.contains("Annotations"));
    assert!(!controls.contains("Inbox"));
    let library = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .library
        .context("Missing library")?;
    assert_eq!(library.y, 2);
    Ok(())
}
