//! Ordered input bursts preserve editing and cancel released autorepeats.

mod common;

use anyhow::{Result, anyhow};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use lume::{
    app::{Action, App, Focus, InputBatch},
    model::IncomingArticle,
};

fn key(code: KeyCode, kind: KeyEventKind) -> Event {
    Event::Key(KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind))
}

fn dispatch(app: &mut App, events: Vec<Event>) -> Result<usize> {
    let mut batch = InputBatch::new(events);
    let mut actions = 0;
    while let Some(action) = batch.dispatch_next(app)? {
        assert_eq!(action, Action::None);
        actions += 1;
    }
    Ok(actions)
}

fn articles() -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let rows = (0..300)
        .map(|index| IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: format!("navigation-{index}"),
            title: format!("Article {index}"),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000 + index,
            body: "A cached computing article.".into(),
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &rows)?;
    Ok((directory, App::new(store, false)?))
}

#[test]
fn navigation_uses_the_storage_limit_for_zero_and_oversized_page_requests()
-> Result<()> {
    let (_directory, mut app) = articles()?;
    let id = app
        .current()
        .ok_or_else(|| anyhow!("Missing first article"))?
        .id;
    app.page_size = 0;
    app.jump_article(id)?;
    assert_eq!(app.library.articles.len(), 1);
    app.focus = Focus::Articles;
    dispatch(&mut app, vec![key(KeyCode::Down, KeyEventKind::Press)])?;
    assert_eq!(app.page_offset, 1);
    assert_eq!(app.selected, 0);

    app.page_size = usize::MAX;
    app.jump_article(id)?;
    app.focus = Focus::Articles;
    dispatch(&mut app, vec![key(KeyCode::End, KeyEventKind::Press)])?;
    assert_eq!(app.page_offset, 0);
    assert_eq!(app.selected, 300);
    assert_eq!(
        app.current().map(|article| article.title.as_str()),
        Some("Cache & consequence")
    );
    Ok(())
}

#[test]
fn legacy_navigation_burst_loads_only_its_final_page_without_losing_distance()
-> Result<()> {
    let (_directory, mut app) = articles()?;
    let events = (0..220)
        .map(|_| key(KeyCode::Down, KeyEventKind::Press))
        .collect();

    let actions = dispatch(&mut app, events)?;

    assert_eq!(actions, 1);
    assert_eq!(app.page_offset, 200);
    assert_eq!(app.selected, 20);
    assert_eq!(
        app.current().map(|article| article.title.as_str()),
        Some("Article 79")
    );
    Ok(())
}

#[test]
fn release_cancels_pending_repeats_but_a_quick_tap_still_moves_once()
-> Result<()> {
    let (_directory, mut app) = articles()?;
    let mut events = vec![key(KeyCode::Down, KeyEventKind::Press)];
    events.extend((0..100).map(|_| key(KeyCode::Down, KeyEventKind::Repeat)));
    events.push(key(KeyCode::Down, KeyEventKind::Release));

    dispatch(&mut app, events)?;
    assert_eq!(app.selected, 1);
    let mut repeats = (0..100)
        .map(|_| key(KeyCode::Down, KeyEventKind::Repeat))
        .collect::<Vec<_>>();
    repeats.push(Event::Resize(120, 32));
    repeats.push(key(KeyCode::Down, KeyEventKind::Release));
    dispatch(&mut app, repeats)?;

    assert_eq!(app.selected, 1);
    Ok(())
}

#[test]
fn distinct_taps_and_direction_changes_keep_their_order() -> Result<()> {
    let (_directory, mut app) = articles()?;
    let mut events = Vec::new();
    for _ in 0..3 {
        events.push(key(KeyCode::Down, KeyEventKind::Press));
        events.push(key(KeyCode::Down, KeyEventKind::Release));
    }
    events.push(key(KeyCode::Up, KeyEventKind::Press));
    events.push(key(KeyCode::Up, KeyEventKind::Release));

    dispatch(&mut app, events)?;

    assert_eq!(app.selected, 2);
    Ok(())
}

#[test]
fn opening_an_editor_in_a_burst_preserves_repeated_text_and_paste() -> Result<()>
{
    let (_directory, mut app) = articles()?;
    let mut events = vec![key(KeyCode::Char('n'), KeyEventKind::Press)];
    events
        .extend((0..4).map(|_| key(KeyCode::Char('j'), KeyEventKind::Repeat)));
    events.push(key(KeyCode::Char('j'), KeyEventKind::Release));
    events.push(Event::Paste(" ação".into()));
    events.push(Event::Key(KeyEvent::new(
        KeyCode::Char('s'),
        KeyModifiers::CONTROL,
    )));

    dispatch(&mut app, events)?;

    assert_eq!(app.comments[0].body, "jjjj ação");
    Ok(())
}

#[test]
fn quit_is_delivered_after_navigation_without_discarding_it() -> Result<()> {
    let (_directory, mut app) = articles()?;
    let mut batch = InputBatch::new(vec![
        key(KeyCode::Down, KeyEventKind::Press),
        key(KeyCode::Down, KeyEventKind::Repeat),
        key(KeyCode::Char('q'), KeyEventKind::Press),
    ]);
    assert_eq!(batch.dispatch_next(&mut app)?, Some(Action::None));
    assert_eq!(app.selected, 2);
    assert_eq!(batch.dispatch_next(&mut app)?, Some(Action::Quit));
    assert!(batch.dispatch_next(&mut app)?.is_none());
    Ok(())
}

#[test]
fn sidebar_counts_remain_valid_across_view_changes_and_boundary_keys()
-> Result<()> {
    let (_directory, mut app) = articles()?;
    let before = app
        .navigation
        .iter()
        .map(|item| item.count)
        .collect::<Vec<_>>();
    app.focus = Focus::Library;
    dispatch(&mut app, vec![key(KeyCode::Down, KeyEventKind::Press)])?;
    dispatch(&mut app, vec![key(KeyCode::Up, KeyEventKind::Press)])?;
    dispatch(&mut app, vec![key(KeyCode::Up, KeyEventKind::Press)])?;
    assert_eq!(app.nav_selected, 0);
    let after = app
        .navigation
        .iter()
        .map(|item| item.count)
        .collect::<Vec<_>>();
    assert_eq!(before, after);
    let article = app
        .current()
        .ok_or_else(|| anyhow!("Article disappeared"))?;
    assert_eq!(article.title, "Article 299");
    Ok(())
}

#[test]
fn word_and_horizontal_releases_cancel_repeats_without_losing_the_next_command()
-> Result<()> {
    let (_directory, mut app) = articles()?;
    app.focus = Focus::Reader;
    let mut events = vec![key(KeyCode::Char('w'), KeyEventKind::Press)];
    events.extend(
        (0..100).map(|_| key(KeyCode::Char('w'), KeyEventKind::Repeat)),
    );
    events.push(key(KeyCode::Char('w'), KeyEventKind::Release));
    dispatch(&mut app, events)?;
    assert_eq!(app.cursor_offset(), 2);

    let mut events = vec![key(KeyCode::Right, KeyEventKind::Press)];
    events.extend((0..100).map(|_| key(KeyCode::Right, KeyEventKind::Repeat)));
    events.push(key(KeyCode::Right, KeyEventKind::Release));
    events.push(key(KeyCode::Char('?'), KeyEventKind::Press));
    dispatch(&mut app, events)?;

    assert_eq!(app.cursor_offset(), 3);
    assert!(matches!(app.mode, lume::app::Mode::Help));
    Ok(())
}
