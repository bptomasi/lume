//! Panel-scoped filtering preserves identities and shares pointer geometry.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    model::View,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 160, 44);

#[test]
fn mouse_scroll_and_click_use_filtered_library_identities() -> Result<()> {
    use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
    let (_directory, mut app) = application()?;
    app.focus = Focus::Library;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("security")?;
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .library
        .context("Library hidden")?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::ScrollDown,
        column: area.x + 4,
        row: area.y + 4,
        modifiers: KeyModifiers::NONE,
    })?;
    assert_eq!(app.navigation[app.nav_selected].title, "Security reviews");
    let expected = app.navigation[app.nav_selected].view.clone();
    let row = (area.y..area.bottom())
        .find(|row| {
            ui::navigation_at(&app, area, *row) == Some(app.nav_selected)
        })
        .context("Filtered row missing")?;
    app.mouse(MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.x + 4,
        row,
        modifiers: KeyModifiers::NONE,
    })?;
    assert_eq!(app.view, expected);
    assert!(matches!(app.mode, Mode::Normal));
    Ok(())
}

fn key(app: &mut App, code: KeyCode) -> Result<()> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))?;
    Ok(())
}

fn application() -> Result<(tempfile::TempDir, App)> {
    let (directory, store, _) = common::seeded_store()?;
    store.create_collection("Security research")?;
    store.create_collection("Security reviews")?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

fn two_article_application() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut app) = application()?;
    let mut store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = store.feeds()?.remove(0);
    let parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    let mut second = parsed.articles.into_iter().next().context("Article")?;
    second.external_id.push_str("-second");
    second.url.push_str("-second");
    store.ingest(&feed, &feed.title, &[second])?;
    app.reload()?;
    Ok((directory, app))
}

#[test]
fn library_search_filters_locally_and_commits_a_captured_destination()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let before = app.view.clone();
    app.focus = Focus::Library;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("SECURITY")?;
    assert_eq!(app.view, before);
    assert_eq!(app.library_query(), "SECURITY");
    assert_eq!(app.navigation[app.nav_selected].title, "Security research");
    key(&mut app, KeyCode::Down)?;
    assert_eq!(app.navigation[app.nav_selected].title, "Security reviews");
    let expected = app.navigation[app.nav_selected].view.clone();
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.view, expected);
    assert_eq!(app.focus, Focus::Articles);
    assert!(matches!(app.mode, Mode::Normal));
    app.focus = Focus::Library;
    key(&mut app, KeyCode::Char('k'))?;
    assert_eq!(app.navigation[app.nav_selected].title, "Security research");
    key(&mut app, KeyCode::Esc)?;
    assert!(app.library_query().is_empty());
    assert_eq!(app.focus, Focus::Library);
    Ok(())
}

#[test]
fn empty_library_matches_are_safe_and_cancel_restores_selection() -> Result<()>
{
    let (_directory, mut app) = application()?;
    app.focus = Focus::Library;
    let selected = app.nav_selected;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("no matching destination")?;
    key(&mut app, KeyCode::Down)?;
    assert!(
        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
            .is_err()
    );
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.nav_selected, selected);
    assert!(app.library_query().is_empty());
    Ok(())
}

#[test]
fn inline_article_search_keeps_its_scope_and_pointer_rows() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.set_view(View::All)?;
    app.focus = Focus::Articles;
    key(&mut app, KeyCode::Char('/'))?;
    for character in "cache".chars() {
        key(&mut app, KeyCode::Char(character))?;
        assert_eq!(app.page_total, 1);
    }
    assert_eq!(app.view, View::All);
    assert_eq!(app.page_total, 1);
    let articles = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .articles
        .context("Articles hidden")?;
    let first = articles.y + 3;
    assert_eq!(ui::article_at(&app, articles, first - 2), None);
    assert_eq!(ui::article_at(&app, articles, first), Some(0));
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    let buffer = terminal.backend().buffer();
    assert_eq!(buffer[(articles.x + 1, articles.y + 1)].symbol(), "/");
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.search, "cache");
    key(&mut app, KeyCode::Esc)?;
    assert!(app.search.is_empty());
    assert_eq!(app.view, View::All);
    Ok(())
}

#[test]
fn background_article_search_filters_before_enter_and_replaces_old_queries()
-> Result<()> {
    let (directory, mut app) = application()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.focus = Focus::Articles;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("does-not-exist")?;
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))?;
    for character in "cache".chars() {
        key(&mut app, KeyCode::Char(character))?;
    }
    common::settle_notes(&mut app)?;
    assert_eq!(app.search, "cache");
    assert_eq!(app.page_total, 1);
    assert!(matches!(app.mode, Mode::Input(_)));
    assert_eq!(
        app.current().context("Live result missing")?.title,
        "Cache & consequence"
    );
    key(&mut app, KeyCode::Esc)?;
    common::settle_notes(&mut app)?;
    assert!(app.search.is_empty());
    app.end_session()?;
    Ok(())
}

#[test]
fn escape_removes_reader_search_without_leaving_fullscreen_or_reloading()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.focus = Focus::Reader;
    key(&mut app, KeyCode::F(11))?;
    assert!(app.workspace.fullscreen);
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("copy")?;
    key(&mut app, KeyCode::Enter)?;
    assert!(!app.reader_tools.hits.is_empty());
    let text = app.reader_text.clone();
    let selected = app.current().context("Article missing")?.id;
    key(&mut app, KeyCode::Esc)?;
    assert!(app.reader_tools.hits.is_empty());
    assert!(app.reader_tools.needle.is_empty());
    assert!(app.workspace.fullscreen);
    assert_eq!(app.focus, Focus::Reader);
    assert_eq!(app.reader_text, text);
    assert_eq!(app.current().context("Selection lost")?.id, selected);
    key(&mut app, KeyCode::Esc)?;
    assert!(!app.workspace.fullscreen);
    assert_eq!(app.focus, Focus::Reader);
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.focus, Focus::Articles);
    assert_eq!(app.current().context("Selection lost")?.id, selected);
    Ok(())
}

#[test]
fn reader_escape_clears_local_search_before_leaving_and_clearing_filter()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.focus = Focus::Articles;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("cache")?;
    key(&mut app, KeyCode::Enter)?;
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.focus, Focus::Reader);
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("copy")?;
    key(&mut app, KeyCode::Enter)?;
    assert!(!app.reader_tools.hits.is_empty());

    key(&mut app, KeyCode::Esc)?;
    assert!(app.reader_tools.needle.is_empty());
    assert!(app.reader_tools.hits.is_empty());
    assert_eq!(app.search, "cache");
    assert_eq!(app.focus, Focus::Reader);
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.focus, Focus::Articles);
    assert!(app.search.is_empty());
    Ok(())
}

#[test]
fn reader_escape_without_a_search_preserves_the_loaded_article() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.focus = Focus::Reader;
    let selected = app.current().context("Article missing")?.id;
    let text = app.reader_text.clone();

    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.focus, Focus::Articles);
    assert_eq!(app.current().context("Selection lost")?.id, selected);
    assert_eq!(app.reader_text, text);
    Ok(())
}

#[test]
fn reader_escape_reveals_articles_in_collapsed_and_zen_layouts() -> Result<()> {
    for zen in [false, true] {
        let (_directory, mut app) = application()?;
        app.focus = Focus::Reader;
        app.workspace.show_articles = false;
        app.workspace.zen = zen;

        key(&mut app, KeyCode::Esc)?;
        assert_eq!(app.focus, Focus::Articles);
        assert!(app.workspace.show_articles);
        assert!(!app.workspace.zen);
        assert!(
            ui::workspace_areas(SCREEN, app.focus, &app.workspace)
                .articles
                .is_some()
        );
    }
    Ok(())
}

#[test]
fn query_cursor_motion_preserves_selection_and_does_not_start_navigation()
-> Result<()> {
    for background in [false, true] {
        let (directory, mut app) = two_article_application()?;
        if background {
            app.enable_background_navigation(
                &directory.path().join("library.sqlite3"),
            )?;
        }
        app.focus = Focus::Articles;
        key(&mut app, KeyCode::Char('/'))?;
        app.paste("is:unread")?;
        common::settle_notes(&mut app)?;
        assert_eq!(app.page_total, 2);
        key(&mut app, KeyCode::Down)?;
        common::settle_notes(&mut app)?;
        let selected = app.current().context("Second result missing")?.id;

        for code in [KeyCode::Left, KeyCode::Right, KeyCode::Home, KeyCode::End]
        {
            key(&mut app, code)?;
            assert!(!app.navigation_loading, "Caret motion queried storage");
            assert_eq!(app.selected, 1);
            assert_eq!(app.current().context("Selection lost")?.id, selected);
        }
        key(&mut app, KeyCode::Enter)?;
        common::settle_notes(&mut app)?;
        key(&mut app, KeyCode::Char('/'))?;
        key(&mut app, KeyCode::Esc)?;
        assert!(
            !app.navigation_loading,
            "Unchanged cancellation queried storage"
        );
        assert_eq!(app.selected, 1);
        assert_eq!(app.current().context("Selection lost")?.id, selected);
        app.end_session()?;
    }
    Ok(())
}

#[test]
fn catalog_refresh_preserves_an_uncommitted_library_match() -> Result<()> {
    let (directory, mut app) = application()?;
    let original = app.view.clone();
    app.focus = Focus::Library;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("security")?;
    key(&mut app, KeyCode::Down)?;
    let selected = app.navigation[app.nav_selected].view.clone();
    assert_ne!(selected, original);

    let store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    store.create_collection("Security architecture")?;
    app.reload()?;

    assert_eq!(app.view, original, "Refresh applied an uncommitted filter");
    assert_eq!(app.navigation[app.nav_selected].view, selected);
    key(&mut app, KeyCode::Enter)?;
    assert_eq!(app.view, selected);
    Ok(())
}

#[test]
fn async_search_publishes_titles_without_loading_the_selected_body()
-> Result<()> {
    let (directory, mut app) = two_article_application()?;
    let body = app.body.clone();
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.focus = Focus::Articles;
    key(&mut app, KeyCode::Char('/'))?;
    let visible = app.visible.len();
    app.paste("cache")?;
    assert_eq!(app.visible.len(), visible, "Typing blanked the list");
    common::settle_notes(&mut app)?;
    key(&mut app, KeyCode::Down)?;
    assert!(!app.navigation_loading, "Preview started a reader load");
    assert_eq!(app.body, body);
    let selected = app.current().context("Preview selection")?.id;
    key(&mut app, KeyCode::Enter)?;
    common::settle_notes(&mut app)?;
    assert_eq!(app.current().context("Committed selection")?.id, selected);
    assert!(!app.reader_stale());
    Ok(())
}

#[test]
fn sidebar_backspace_and_replacement_expand_matches_again() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.focus = Focus::Library;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("Security reviews")?;
    assert_eq!(app.navigation[app.nav_selected].title, "Security reviews");
    for _ in 0..7 {
        key(&mut app, KeyCode::Backspace)?;
    }
    key(&mut app, KeyCode::Up)?;
    assert_eq!(app.navigation[app.nav_selected].title, "Security research");
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::CONTROL))?;
    app.paste("Inbox")?;
    assert_eq!(app.navigation[app.nav_selected].title, "Inbox");
    Ok(())
}

#[test]
fn beginning_article_search_saves_the_displayed_reading_position() -> Result<()>
{
    let (directory, mut app) = application()?;
    app.resize_reader(12, 2);
    app.scroll = 2;
    let expected = app.line_ranges[app.scroll].start;
    let id = app.current().context("Article")?.id;
    app.focus = Focus::Articles;
    key(&mut app, KeyCode::Char('/'))?;
    let store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.progress(id)?, expected);
    Ok(())
}

#[test]
fn moving_during_a_pending_query_does_not_lose_results_after_page_shrink()
-> Result<()> {
    let (directory, mut app) = two_article_application()?;
    let mut store =
        lume::store::Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = store.feeds()?.remove(0);
    let mut incoming =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    incoming.articles[0].external_id.push_str("-second");
    incoming.articles[0].url.push_str("-second");
    incoming.articles[0].title = "Other result".into();
    incoming.articles[0].body = "An unrelated publisher text.".into();
    store.ingest(&feed, &feed.title, &incoming.articles)?;
    app.reload()?;
    app.page_size = 1;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.focus = Focus::Articles;
    key(&mut app, KeyCode::Char('/'))?;
    app.paste("title:cache")?;
    key(&mut app, KeyCode::Down)?;
    common::settle_notes(&mut app)?;
    assert_eq!(app.page_total, 1);
    assert_eq!(app.page_offset, 0);
    assert_eq!(
        app.current().context("Live result disappeared")?.title,
        "Cache & consequence"
    );
    Ok(())
}
