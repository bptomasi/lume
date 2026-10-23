//! Article-card selection is a preview; reading state requires explicit intent.
mod common;

use anyhow::{Context, Result};
use crossterm::event::{
    KeyCode, KeyModifiers, MouseButton, MouseEvent, MouseEventKind,
};
use lume::{
    app::{App, Focus},
    model::View,
    source,
    store::Store,
    ui,
};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

const SCREEN: Rect = Rect::new(0, 0, 160, 44);

fn application(background: bool) -> Result<(tempfile::TempDir, App, Store)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].external_id = "second".into();
    parsed.articles[0].url = "https://journal.example/second".into();
    parsed.articles[0].title = "Second preview".into();
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    store.clear_history()?;
    let path = directory.path().join("library.sqlite3");
    let observed = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Articles;
    app.resize_workspace(SCREEN);
    if background {
        app.enable_background_navigation(&path)?;
    }
    settle(&mut app)?;
    Ok((directory, app, observed))
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        app.poll_note_workspace()?;
        app.poll_navigation()?;
        app.poll_reader();
        if !app.notes_pending()
            && !app.catalog_pending()
            && !app.article_preparing()
        {
            return Ok(());
        }
        anyhow::ensure!(Instant::now() < deadline, "Article did not settle");
        std::thread::sleep(Duration::from_millis(1));
    }
}

fn click(app: &mut App, selected: usize) -> Result<()> {
    let area = ui::workspace_areas(SCREEN, app.focus, &app.workspace)
        .articles
        .context("Articles hidden")?;
    let row = (area.y + 1..area.bottom() - 1)
        .find(|row| ui::article_at(app, area, *row) == Some(selected))
        .context("Card hidden")?;
    for kind in [
        MouseEventKind::Down(MouseButton::Left),
        MouseEventKind::Up(MouseButton::Left),
    ] {
        app.mouse(MouseEvent {
            kind,
            column: area.x + 3,
            row,
            modifiers: KeyModifiers::NONE,
        })?;
    }
    Ok(())
}

#[test]
fn clicking_cards_keeps_them_unread_and_preserves_article_focus() -> Result<()>
{
    for background in [false, true] {
        for compact in [false, true] {
            let (_directory, mut app, observed) = application(background)?;
            app.workspace.compact = compact;
            click(&mut app, 1)?;
            assert_eq!(app.focus, Focus::Articles);
            assert_eq!(app.selected, 1);
            assert!(!app.notes_pending(), "Click queued a personal mutation");
            settle(&mut app)?;
            assert!(app.current().context("Preview")?.unread);
            assert_eq!(observed.view_count(&View::Inbox)?, 2);
            assert!(!observed.undo()?, "Click created an undo entry");
            app.end_session()?;
        }
    }
    Ok(())
}

#[test]
fn clicking_the_ready_selection_does_not_enqueue_another_source_load()
-> Result<()> {
    let (_directory, mut app, observed) = application(true)?;
    click(&mut app, 0)?;
    assert!(!app.navigation_loading);
    assert!(!app.catalog_pending());
    assert!(!app.notes_pending());
    assert_eq!(observed.view_count(&View::Inbox)?, 2);
    app.end_session()
}

#[test]
fn enter_after_a_mouse_preview_opens_without_marking_the_article_read()
-> Result<()> {
    let (_directory, mut app, observed) = application(true)?;
    click(&mut app, 1)?;
    settle(&mut app)?;
    let target = app.current().context("Selected preview")?.id;
    assert!(observed.article(target)?.unread);
    app.key(KeyCode::Enter.into())?;
    settle(&mut app)?;
    assert_eq!(app.focus, Focus::Reader);
    assert!(observed.article(target)?.unread);
    assert_eq!(app.current().map(|item| item.id), Some(target));
    app.end_session()
}
