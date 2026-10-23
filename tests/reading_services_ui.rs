//! Real key paths qualify background results, cancellation and read-only web pages.

mod common;

use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode},
    model::WebDocument,
};

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn command(app: &mut App, command: &str) -> Result<()> {
    key(app, KeyCode::Char(':'))?;
    app.paste(command)?;
    key(app, KeyCode::Enter)
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        app.poll_maintenance()?;
        app.poll_web()?;
        app.poll_picker()?;
        app.poll_navigation()?;
        app.poll_note_workspace()?;
        if !app.maintenance_pending()
            && !app.web_pending()
            && !app.picker_pending()
            && !app.navigation_loading
            && !app.notes_pending()
        {
            return Ok(());
        }
        ensure!(Instant::now() < deadline, "Reading service did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn open_link_command_follows_local_sections_in_a_linked_page() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    store.save_web_document(&WebDocument {
        url: "https://example.org/sections".into(),
        title: "Sections".into(),
        body: "[Jump](#target)\n\n## Target\n\nDestination.".into(),
    })?;
    let mut app = App::new(store, false)?;
    command(&mut app, "open-page https://example.org/sections")?;
    settle(&mut app)?;
    assert_eq!(app.web_page().context("Page")?.cursor, 0);
    command(&mut app, "open-link")?;
    let page = app.web_page().context("Page")?;
    assert!(page.cursor > 0);
    assert!(page.rendered.text[page.cursor..].starts_with("Target"));
    Ok(())
}

#[test]
fn missing_section_preserves_readable_page_and_original_browser_target()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    store.save_web_document(&WebDocument {
        url: "https://example.org/sections".into(),
        title: "Sections".into(),
        body: "# Readable page\n\nContent remains available.".into(),
    })?;
    let mut app = App::new(store, false)?;
    let target = "https://example.org/sections#removed";
    command(&mut app, &format!("open-page {target}"))?;
    settle(&mut app)?;
    assert!(
        app.web_page()
            .context("Page")?
            .rendered
            .text
            .contains("Content remains")
    );
    assert!(app.status.contains("Section unavailable"));
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 100, 40));
    settle(&mut app)?;
    let action =
        app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE))?;
    assert_eq!(action, lume::app::Action::Open(target.into()));
    Ok(())
}

#[test]
fn linked_pages_leave_parent_notes_and_cursor_intact_and_back_restores_them()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let document = WebDocument {
        url: "https://example.org/linked".into(),
        title: "Linked content".into(),
        body: "# Linked content\n\nParagraph with [section](#target).\n\n## Target\n\nDestination.".into(),
    };
    store.save_web_document(&document)?;
    let mut app = App::new(store, false)?;
    let parent = app.current().context("Parent")?.id;
    let position = app.cursor_offset();

    command(&mut app, "open-page https://example.org/linked")?;
    settle(&mut app)?;
    assert!(app.web_active());
    assert!(
        app.web_page()
            .context("Web page")?
            .rendered
            .text
            .contains("Linked content")
    );
    assert!(command(&mut app, "note").is_err());
    key(&mut app, KeyCode::Esc)?;
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;

    assert!(!app.web_active());
    assert_eq!(app.current().context("Parent")?.id, parent);
    assert_eq!(app.cursor_offset(), position);
    Ok(())
}

#[test]
fn cache_cleanup_requires_confirmation_and_closed_reports_never_reopen()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "storage clean all")?;
    assert!(
        matches!(&app.mode, Mode::Document(document) if document.confirm.is_some())
    );
    assert!(!app.maintenance_pending());
    key(&mut app, KeyCode::Enter)?;
    assert!(app.maintenance_pending());
    key(&mut app, KeyCode::Esc)?;

    settle(&mut app)?;

    assert!(matches!(app.mode, Mode::Normal));
    command(&mut app, "performance on")?;
    assert!(
        matches!(&app.mode, Mode::Document(document) if document.lines.iter().any(|line| line.contains("enabled")))
    );
    Ok(())
}

#[test]
fn cached_recommendations_propose_a_session_without_marking_articles_read()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let reader =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    let mut profile = store.recommendation_profile()?;
    profile.interests.push(lume::model::Interest {
        name: "Cache".into(),
        weight: 8,
        query: "cache".into(),
    });
    store.save_recommendation_profile(&profile)?;
    store.refresh_recommendations()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "session 20")?;
    settle(&mut app)?;
    assert!(
        matches!(&app.mode, Mode::Document(document) if document.confirm.is_some())
    );
    assert!(reader.reading_session()?.is_none());
    key(&mut app, KeyCode::Enter)?;
    settle(&mut app)?;

    let session = reader.reading_session()?.context("Saved session")?;
    assert_eq!(session.items.len(), 1);
    assert!(reader.article(session.items[0].article)?.unread);
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "session next")?;
    settle(&mut app)?;
    assert_eq!(reader.reading_session()?.context("Session")?.position, 1);
    Ok(())
}

#[test]
fn rapid_selector_queries_and_reopening_keep_only_current_results() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    key(&mut app, KeyCode::Char(' '))?;
    key(&mut app, KeyCode::Char('f'))?;
    for _ in 0..100 {
        app.paste("x")?;
        key(&mut app, KeyCode::Backspace)?;
    }
    app.paste("no-such-result-unique")?;
    settle(&mut app)?;
    assert!(
        matches!(&app.mode, Mode::Picker(picker) if picker.choices.is_empty() && picker.preview.contains("No matches")),
        "Unexpected selector: {:?}",
        app.mode
    );
    key(&mut app, KeyCode::Esc)?;

    key(&mut app, KeyCode::Char(' '))?;
    key(&mut app, KeyCode::Char('f'))?;
    app.paste("cache")?;
    settle(&mut app)?;

    let Mode::Picker(picker) = &app.mode else {
        anyhow::bail!("Selector closed unexpectedly");
    };
    assert_eq!(picker.choices.len(), 1);
    assert!(picker.preview.contains("measure"));
    assert_eq!(picker.query, "cache");
    app.end_session()?;
    Ok(())
}

#[test]
fn linked_fragments_and_reflow_preserve_local_history_and_selection()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    store.save_web_document(&WebDocument {
        url: "https://example.org/sections".into(),
        title: "Sections".into(),
        body: "[Jump](#target)\n\n## Target\n\nWords at the destination."
            .into(),
    })?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 120, 40));
    command(&mut app, "open-page https://example.org/sections")?;
    settle(&mut app)?;
    key(&mut app, KeyCode::Enter)?;
    let page = app.web_page().context("Page")?;
    assert_eq!(&page.rendered.text[page.cursor..][..6], "Target");
    key(&mut app, KeyCode::Char('v'))?;
    key(&mut app, KeyCode::Char('l'))?;

    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 50, 30));
    app.poll_web()?;
    settle(&mut app)?;

    assert!(app.web_page().context("Page")?.selection.is_some());
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(app.web_page().context("Page")?.cursor, 0);
    assert!(app.web_active());
    Ok(())
}

#[test]
fn linked_pages_and_reports_render_at_small_sizes_without_overflow()
-> Result<()> {
    use ratatui::{Terminal, backend::TestBackend};
    let (_directory, store, _) = common::seeded_store()?;
    store.save_web_document(&WebDocument {
        url: "https://example.org/small".into(),
        title: "Small page".into(),
        body: "# Small page\n\nUnicode café and 界 text.\n\n```sql\nSELECT 42;\n```".into(),
    })?;
    let mut app = App::new(store, false)?;
    for (width, height) in [(1, 1), (8, 3), (40, 12), (120, 40)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        app.resize_workspace(ratatui::layout::Rect::new(0, 0, width, height));
        command(&mut app, "open-page https://example.org/small")?;
        settle(&mut app)?;
        terminal.draw(|frame| lume::ui::draw(frame, &app))?;
        assert_eq!(
            terminal.backend().buffer().content.len(),
            usize::from(width) * usize::from(height)
        );
        key(&mut app, KeyCode::Esc)?;
    }
    Ok(())
}

#[test]
fn linked_mouse_scroll_is_one_row_and_drag_yanks_the_exact_selection()
-> Result<()> {
    use crossterm::event::{MouseButton, MouseEventKind};
    let (_directory, mut app, area) = linked_mouse_page()?;
    mouse(&mut app, MouseEventKind::ScrollDown, area.x, area.y)?;
    assert_eq!(app.web_page().context("Page")?.scroll, 1);
    mouse(&mut app, MouseEventKind::ScrollUp, area.x, area.y)?;
    assert_eq!(app.web_page().context("Page")?.scroll, 0);
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        area.x,
        area.y,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        area.x + 4,
        area.y,
    )?;
    mouse(
        &mut app,
        MouseEventKind::Up(MouseButton::Left),
        area.x + 4,
        area.y,
    )?;
    assert_eq!(
        app.key(KeyEvent::new(KeyCode::Char('y'), KeyModifiers::NONE))?,
        lume::app::Action::Copy("alpha".into())
    );
    Ok(())
}

fn mouse(
    app: &mut App,
    kind: crossterm::event::MouseEventKind,
    column: u16,
    row: u16,
) -> Result<()> {
    app.mouse(crossterm::event::MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::NONE,
    })?;
    Ok(())
}

fn linked_mouse_page() -> Result<(tempfile::TempDir, App, ratatui::layout::Rect)>
{
    let (directory, store, _) = common::seeded_store()?;
    store.save_web_document(&WebDocument {
        url: "https://example.org/mouse".into(),
        title: "Mouse page".into(),
        body: "alpha beta gamma\n\n".repeat(80),
    })?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 120, 40));
    command(&mut app, "open-page https://example.org/mouse")?;
    settle(&mut app)?;
    let area = lume::ui::web_content(
        ratatui::layout::Rect::new(0, 0, 120, 40),
        app.web_page().context("Page")?.width,
    );
    Ok((directory, app, area))
}

#[test]
fn coalesced_runtime_wheel_events_keep_their_exact_distance_in_linked_pages()
-> Result<()> {
    use crossterm::event::{Event, MouseEvent, MouseEventKind};
    let (_directory, mut app, area) = linked_mouse_page()?;
    let events = (0..3)
        .map(|_| {
            Event::Mouse(MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: area.x,
                row: area.y,
                modifiers: KeyModifiers::NONE,
            })
        })
        .collect();
    let mut batch = lume::app::InputBatch::new(events);

    batch.dispatch_next(&mut app)?;

    assert!(batch.is_empty());
    assert_eq!(app.web_page().context("Page")?.scroll, 3);
    Ok(())
}

#[test]
fn a_saved_page_acknowledgement_cannot_replace_a_newer_pending_navigation()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    cache_linked(&store, "first", "First page with retained content.")?;
    cache_linked(&store, "second", "Second page with newer content.")?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;

    command(&mut app, "save-page")?;
    command(&mut app, "open-page https://example.org/second")?;
    acknowledge_personal_writes(&mut app)?;

    assert!(app.web_active(), "Older save closed the new navigation");
    assert!(app.web_pending());
    settle(&mut app)?;
    assert_eq!(
        app.web_page().context("New page")?.document.url,
        "https://example.org/second"
    );
    app.end_session()?;
    Ok(())
}

#[test]
fn mouse_selection_cannot_carry_an_old_byte_anchor_into_a_new_page()
-> Result<()> {
    use crossterm::event::{MouseButton, MouseEventKind};
    let (_directory, store, _) = common::seeded_store()?;
    cache_linked(&store, "first", "ASCII first page content.")?;
    cache_linked(&store, "second", "界界界 Unicode destination.")?;
    let mut app = App::new(store, false)?;
    let screen = ratatui::layout::Rect::new(0, 0, 120, 40);
    app.resize_workspace(screen);
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;
    let area = lume::ui::web_content(
        screen,
        app.web_page().context("First page")?.width,
    );
    mouse(
        &mut app,
        MouseEventKind::Down(MouseButton::Left),
        area.x + 1,
        area.y,
    )?;

    command(&mut app, "open-page https://example.org/second")?;
    settle(&mut app)?;
    mouse(
        &mut app,
        MouseEventKind::Drag(MouseButton::Left),
        area.x + 3,
        area.y,
    )?;

    assert!(
        app.web_page()
            .context("New page")?
            .selected_range()
            .is_none()
    );
    assert!(key(&mut app, KeyCode::Char('y')).is_err());
    Ok(())
}

#[test]
fn back_during_page_loading_discards_the_pending_page_before_local_history()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    cache_linked(
        &store,
        "first",
        "[Jump](#target)\n\n## Target\n\nOriginal destination.",
    )?;
    cache_linked(&store, "second", "New page that must remain cancelled.")?;
    let mut app = App::new(store, false)?;
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;
    key(&mut app, KeyCode::Enter)?;
    let cursor = app.web_page().context("First page")?.cursor;

    command(&mut app, "open-page https://example.org/second")?;
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    settle(&mut app)?;

    let page = app.web_page().context("Previous page stays open")?;
    assert_eq!(page.document.url, "https://example.org/first");
    assert_eq!(page.cursor, cursor);
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    assert_eq!(app.web_page().context("Local jump")?.cursor, 0);
    Ok(())
}

fn cache_linked(
    store: &lume::store::Store,
    name: &str,
    body: &str,
) -> Result<()> {
    store.save_web_document(&WebDocument {
        url: format!("https://example.org/{name}"),
        title: format!("Page {name}"),
        body: body.into(),
    })?;
    Ok(())
}

#[test]
fn following_a_link_during_navigation_cannot_use_the_previous_page()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    cache_linked(&store, "first", "[Jump](#target)\n\n## Target\n\nOld text.")?;
    cache_linked(&store, "second", "# New page\n\nCurrent text.")?;
    let mut app = App::new(store, false)?;
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;
    command(&mut app, "open-page https://example.org/second")?;
    assert!(command(&mut app, "open-link").is_err());
    key(&mut app, KeyCode::Esc)?;
    settle(&mut app)?;
    assert_eq!(
        app.web_page().context("Page")?.document.url,
        "https://example.org/second"
    );
    Ok(())
}

fn acknowledge_personal_writes(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.notes_pending() {
        app.poll_note_tasks()?;
        ensure!(Instant::now() < deadline, "Personal writes did not finish");
        std::thread::yield_now();
    }
    Ok(())
}

#[test]
fn saving_a_page_cannot_close_a_later_visit_to_the_same_address() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    cache_linked(&store, "first", "Original content to save and revisit.")?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;

    command(&mut app, "save-page")?;
    key(&mut app, KeyCode::Esc)?;
    command(&mut app, "open-page https://example.org/first")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.web_pending() {
        app.poll_web()?;
        ensure!(Instant::now() < deadline, "Revisited page did not load");
        std::thread::yield_now();
    }
    acknowledge_personal_writes(&mut app)?;

    assert!(app.web_active());
    assert_eq!(
        app.web_page().context("Revisited page")?.document.url,
        "https://example.org/first"
    );
    app.end_session()?;
    Ok(())
}

#[test]
fn a_failed_page_open_preserves_reflow_of_the_previous_page() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    cache_linked(&store, "first", &"Readable previous content. ".repeat(100))?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 120, 40));
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;

    app.resize_workspace(ratatui::layout::Rect::new(0, 0, 50, 30));
    command(&mut app, "open-page https://example.org/missing")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if app.poll_web().is_err() {
            break;
        }
        ensure!(Instant::now() < deadline, "Offline page did not fail");
        std::thread::yield_now();
    }
    settle(&mut app)?;

    let page = app.web_page().context("Previous page")?;
    assert_eq!(page.document.url, "https://example.org/first");
    assert_eq!(page.width, 44);
    Ok(())
}

#[test]
fn saving_during_navigation_cannot_capture_the_previous_visible_page()
-> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    cache_linked(&store, "first", "Previous visible content.")?;
    cache_linked(&store, "second", "New content that is still loading.")?;
    let mut app = App::new(store, false)?;
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;

    command(&mut app, "open-page https://example.org/second")?;

    assert!(command(&mut app, "save-page").is_err());
    assert!(app.web_active());
    assert!(app.web_pending());
    Ok(())
}

#[test]
fn holding_enter_on_a_link_does_not_start_repeated_resource_opens() -> Result<()>
{
    let (_directory, store, _) = common::seeded_store()?;
    cache_linked(
        &store,
        "first",
        "[Resource](https://example.org/file.pdf)\n\nReadable context.",
    )?;
    let mut app = App::new(store, false)?;
    command(&mut app, "open-page https://example.org/first")?;
    settle(&mut app)?;

    app.key(KeyEvent {
        kind: crossterm::event::KeyEventKind::Repeat,
        ..KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    })?;

    assert!(!app.web_pending(), "Repeated Enter started another request");
    assert_eq!(
        app.web_page().context("Current page")?.document.url,
        "https://example.org/first"
    );
    Ok(())
}

#[test]
fn formula_preference_applies_to_linked_pages_and_back_history() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    for name in ["first", "second"] {
        store.save_web_document(&WebDocument {
            url: format!("https://example.org/{name}"),
            title: name.into(),
            body: format!("# {name}\n\n$x^2$ and $\\sum_{{i=1}}^n x_i$."),
        })?;
    }
    let mut app = App::new(store, false)?;
    for name in ["first", "second"] {
        command(&mut app, &format!("open-page https://example.org/{name}"))?;
        settle(&mut app)?;
        assert!(app.web_page().context("Page")?.rendered.text.contains("x²"));
    }
    command(&mut app, "latex-formulas off")?;
    settle(&mut app)?;
    assert!(
        app.web_page()
            .context("Page")?
            .rendered
            .text
            .contains("$x^2$")
    );
    app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::CONTROL))?;
    settle(&mut app)?;
    let page = app.web_page().context("Back page")?;
    assert!(page.document.url.ends_with("first"));
    assert!(page.rendered.text.contains("$x^2$"));
    command(&mut app, "latex-formulas on")?;
    settle(&mut app)?;
    assert!(app.web_page().context("Page")?.rendered.text.contains("x²"));
    Ok(())
}

#[test]
fn retention_preview_requires_enter_and_preserves_the_current_reader()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let store = lume::store::Store::open(&path)?;
    rusqlite::Connection::open(&path)?.execute_batch(include_str!(
        "../src/store/sql/test_setup_retention.sql"
    ))?;
    let document = directory.path().join("config.toml");
    let session = lume::config::Session::open(&store, &document)?;
    let mut policy = lume::config::decode(&session.initial().document)?;
    policy.retention.defaults.read_days = lume::config::retention::Age(Some(1));
    std::fs::write(&document, lume::config::encode(&policy)?)?;
    let configured = lume::config::Session::open(&store, &document)?;
    let mut app = App::new(store, false)?;
    app.configure_document(configured.initial().clone());
    app.set_view(lume::model::View::All)?;
    settle(&mut app)?;
    let current = app.current().context("Current reader")?.id;
    command(&mut app, "retention preview")?;
    settle(&mut app)?;
    assert!(
        matches!(&app.mode, Mode::Document(view) if view.confirm.is_some())
    );
    let observed = lume::store::Store::open(&path)?;
    assert_eq!(observed.view_count(&lume::model::View::All)?, 24);
    key(&mut app, KeyCode::Enter)?;
    settle(&mut app)?;
    assert_eq!(observed.view_count(&lume::model::View::All)?, 1);
    assert!(observed.article(current).is_ok());
    assert!(
        matches!(&app.mode, Mode::Document(view) if view.confirm.is_none())
    );
    Ok(())
}

#[test]
fn retention_status_uses_its_own_report_title() -> Result<()> {
    let (_directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "retention status")?;
    settle(&mut app)?;
    assert!(
        matches!(&app.mode, Mode::Document(view) if view.title == "Retention · Esc closes")
    );
    Ok(())
}
