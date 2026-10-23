//! Section controls share keyboard focus, pointer rows and durable fold state.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::{App, Focus},
    model::{SmartFilter, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const SCREEN: Rect = Rect::new(0, 0, 120, 44);

#[test]
fn enter_folds_and_unfolds_each_selected_section_title() -> Result<()> {
    let (_directory, mut app) = application()?;
    for group in ["SMART LISTS", "FEEDS", "SERVERS", "YOUR FOLDERS", "PINNED"] {
        let name = group.to_owned();
        let view = View::Group(name.clone());
        app.set_view(view.clone())?;
        let before = app
            .navigation
            .iter()
            .filter(|item| item.group == group)
            .count();
        assert!(before > 1, "{group} has visible children");
        app.key(KeyCode::Enter.into())?;
        assert!(app.workspace.collapsed_groups.contains(&name));
        assert_eq!(
            app.navigation
                .iter()
                .filter(|item| item.group == group)
                .count(),
            1
        );
        assert_eq!(app.view, view);
        app.key(KeyCode::Enter.into())?;
        assert!(!app.workspace.collapsed_groups.contains(&name));
        assert_eq!(
            app.navigation
                .iter()
                .filter(|item| item.group == group)
                .count(),
            before
        );
        assert_eq!(app.view, view);
    }
    Ok(())
}

#[test]
fn clicking_a_section_title_toggles_once_and_keeps_data_on_reopen() -> Result<()>
{
    let (directory, mut app) = application()?;
    app.set_view(View::Group("SMART LISTS".into()))?;
    click_selected(&mut app)?;
    assert!(
        app.workspace
            .collapsed_groups
            .contains(&"SMART LISTS".into())
    );
    assert_eq!(app.view, View::Group("SMART LISTS".into()));
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(
        store
            .workspace()?
            .collapsed_groups
            .contains(&"SMART LISTS".into())
    );
    assert_eq!(store.smart_lists()?.len(), 1);
    assert_eq!(store.feeds()?.len(), 1);
    assert_eq!(store.servers()?.len(), 1);
    click_selected(&mut app)?;
    assert!(
        !app.workspace
            .collapsed_groups
            .contains(&"SMART LISTS".into())
    );
    Ok(())
}

#[test]
fn arrow_navigation_reaches_section_titles_and_their_children() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.set_view(View::Recommended)?;
    app.key(KeyCode::Down.into())?;
    assert_eq!(app.view, View::Group("YOUR FOLDERS".into()));
    app.key(KeyCode::Down.into())?;
    assert!(matches!(app.view, View::Collection(_)));
    app.key(KeyCode::Up.into())?;
    app.key(KeyCode::Enter.into())?;
    assert!(
        app.workspace
            .collapsed_groups
            .contains(&"YOUR FOLDERS".into())
    );
    app.key(KeyCode::Down.into())?;
    assert_eq!(app.view, View::Group("SMART LISTS".into()));
    Ok(())
}

fn click_selected(app: &mut App) -> Result<()> {
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .library
        .context("Library pane")?;
    let row = (area.y + 1..area.bottom() - 1)
        .find(|row| {
            ui::navigation_at(app, area, *row) == Some(app.nav_selected)
        })
        .context("Visible section control")?;
    let mut terminal =
        Terminal::new(TestBackend::new(SCREEN.width, SCREEN.height))?;
    terminal.draw(|frame| ui::draw(frame, app))?;
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        app.mouse(MouseEvent {
            kind,
            column: area.x + 4,
            row,
            modifiers: KeyModifiers::NONE,
        })?;
    }
    Ok(())
}

fn application() -> Result<(tempfile::TempDir, App)> {
    let (directory, store, _) = common::seeded_store()?;
    store.create_collection("Reading folder")?;
    store.save_smart_list("Saved query", &SmartFilter::default())?;
    store.add_server("https://reader.example", "LUME_TOKEN")?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Library;
    app.set_view(View::Starred)?;
    app.key(KeyCode::Char(':').into())?;
    app.paste("pin")?;
    app.key(KeyCode::Enter.into())?;
    app.set_view(View::All)?;
    app.resize_workspace(SCREEN);
    Ok((directory, app))
}

#[test]
fn folding_uses_cached_counts_even_when_a_catalog_read_would_fail() -> Result<()>
{
    let (directory, mut app) = application()?;
    let path = directory.path().join("library.sqlite3");
    rusqlite::Connection::open(&path)?.execute_batch(include_str!(
        "../src/store/sql/test_unavailable_smart_filters.sql"
    ))?;
    assert!(Store::open(&path)?.smart_lists().is_err());
    for group in ["SMART LISTS", "FEEDS", "SERVERS", "YOUR FOLDERS", "PINNED"] {
        app.set_view(View::Group(group.into()))?;
        let count = app.navigation[app.nav_selected].count;
        for _ in 0..4 {
            app.key(KeyCode::Enter.into())?;
            assert_eq!(app.navigation[app.nav_selected].count, count);
            assert!(!app.navigation_loading);
        }
        click_selected(&mut app)?;
        assert_eq!(app.view, View::Group(group.into()));
    }
    Ok(())
}

#[test]
fn clicking_a_header_clears_the_old_article_without_loading_a_page()
-> Result<()> {
    let (_directory, mut app) = application()?;
    assert!(!app.reader_text.is_empty());
    app.nav_selected = app
        .navigation
        .iter()
        .position(|item| item.view == View::Group("FEEDS".into()))
        .context("Feed header")?;
    click_selected(&mut app)?;
    assert_eq!(app.view, View::Group("FEEDS".into()));
    assert!(app.current().is_none());
    assert!(app.reader_text.is_empty());
    assert_eq!(app.page_total, 0);
    assert!(!app.navigation_loading);
    Ok(())
}

#[test]
fn a_catalog_refresh_updates_counts_while_the_section_stays_folded()
-> Result<()> {
    let (directory, mut app) = application()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let article = store.library()?.articles[0].id;
    app.set_view(View::Group("FEEDS".into()))?;
    app.key(KeyCode::Enter.into())?;
    assert_eq!(app.navigation[app.nav_selected].count, 1);
    store.set_deleted(article, true)?;
    app.reload()?;
    assert_eq!(app.view, View::Group("FEEDS".into()));
    assert_eq!(app.navigation[app.nav_selected].count, 0);
    assert!(app.workspace.collapsed_groups.contains(&"FEEDS".into()));
    app.key(KeyCode::Enter.into())?;
    let feed = app
        .navigation
        .iter()
        .find(|item| matches!(item.view, View::Feed(_)))
        .context("Expanded feed")?;
    assert_eq!(feed.count, 0);
    store.set_deleted(article, false)?;
    app.reload()?;
    assert_eq!(app.navigation[app.nav_selected].count, 1);
    Ok(())
}

#[test]
fn section_selection_cancels_an_older_background_article_request() -> Result<()>
{
    let (directory, mut app) = application()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(View::All)?;
    assert!(app.navigation_loading);
    app.set_view(View::Group("SMART LISTS".into()))?;
    assert!(!app.navigation_loading);
    for _ in 0..4 {
        app.key(KeyCode::Enter.into())?;
        app.poll_navigation()?;
        assert!(!app.catalog_pending());
        assert!(app.current().is_none());
        assert!(app.reader_text.is_empty());
    }
    app.end_session()
}

#[test]
fn unpinning_restores_the_original_section_from_the_cached_catalog()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let feed = app.library.feeds[0].id;
    app.set_view(View::Feed(feed))?;
    run_command(&mut app, "pin")?;
    assert_eq!(app.navigation[app.nav_selected].group, "PINNED");
    run_command(&mut app, "pin")?;
    assert_eq!(app.navigation[app.nav_selected].group, "FEEDS");
    assert_eq!(app.current().context("Article")?.feed_id, feed);
    Ok(())
}

#[test]
fn reordering_keeps_folded_destinations_and_excludes_section_controls()
-> Result<()> {
    let (_directory, mut app) = application()?;
    let smart = app.library.smart_lists[0].id;
    run_command(&mut app, "collapse lists")?;
    app.set_view(View::Feed(app.library.feeds[0].id))?;
    run_command(&mut app, "move-up")?;
    assert!(app.workspace.navigation_order.contains(&View::Smart(smart)));
    assert!(
        !app.workspace
            .navigation_order
            .iter()
            .any(|view| matches!(view, View::Group(_)))
    );
    let article = app.current().context("Selected article")?.id;
    run_command(&mut app, "collapse lists")?;
    assert_eq!(app.current().context("Retained article")?.id, article);
    assert!(
        app.navigation
            .iter()
            .any(|item| item.view == View::Smart(smart))
    );
    Ok(())
}

fn run_command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyCode::Char(':').into())?;
    app.paste(text)?;
    app.key(KeyCode::Enter.into())?;
    Ok(())
}

#[test]
fn enter_on_a_filtered_section_toggles_it_and_keeps_library_focus() -> Result<()>
{
    let (_directory, mut app) = application()?;
    app.key(KeyCode::Char('/').into())?;
    app.paste("FEEDS")?;
    app.key(KeyCode::Enter.into())?;
    assert_eq!(app.focus, Focus::Library);
    assert_eq!(app.view, View::Group("FEEDS".into()));
    assert!(app.workspace.collapsed_groups.contains(&"FEEDS".into()));
    app.key(KeyCode::Enter.into())?;
    assert!(!app.workspace.collapsed_groups.contains(&"FEEDS".into()));
    assert!(
        app.navigation
            .iter()
            .any(|item| matches!(item.view, View::Feed(_)))
    );
    Ok(())
}

#[test]
fn reordering_pins_changes_their_cached_order_without_reloading_the_article()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.set_view(View::All)?;
    run_command(&mut app, "pin")?;
    let article = app.current().context("Selected article")?.id;
    run_command(&mut app, "move-up")?;
    assert_eq!(app.workspace.pins, [View::All, View::Starred]);
    assert_eq!(app.navigation[app.nav_selected].view, View::All);
    assert_eq!(app.current().context("Retained article")?.id, article);
    Ok(())
}

#[test]
fn cached_pin_ranks_ignore_the_previous_sessions_time_boundary() -> Result<()> {
    let (_directory, mut app) = application()?;
    app.workspace.pins.push(View::SinceLastVisit(1));
    app.workspace.last_visit = 42;
    app.reload()?;
    let since = app
        .navigation
        .iter()
        .find(|item| item.view == View::SinceLastVisit(42))
        .context("Current session destination")?;
    assert_eq!(since.group, "PINNED");
    app.set_view(View::Group("PINNED".into()))?;
    app.key(KeyCode::Enter.into())?;
    app.key(KeyCode::Enter.into())?;
    assert!(
        app.navigation.iter().any(|item| item.group == "PINNED"
            && item.view == View::SinceLastVisit(42))
    );
    Ok(())
}

#[test]
fn the_collapse_command_can_replace_a_pending_article_selection() -> Result<()>
{
    let (directory, mut app) = application()?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(View::All)?;
    assert!(app.navigation_loading);
    run_command(&mut app, "collapse feeds")?;
    assert_eq!(app.view, View::Group("FEEDS".into()));
    assert!(!app.navigation_loading);
    assert!(app.workspace.collapsed_groups.contains(&"FEEDS".into()));
    app.end_session()
}
