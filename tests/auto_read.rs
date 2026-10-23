//! Simulated monotonic time qualifies active coverage without slow sleeps.
mod common;
use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode};
use lume::{
    app::{App, Focus, InputBatch},
    config,
    model::View,
    store::Store,
};
use ratatui::layout::Rect;
use std::time::{Duration, Instant};

fn application(body: &str) -> Result<(tempfile::TempDir, App, Store)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].body = body.into();
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    let observed = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    app.focus = Focus::Articles;
    app.key(KeyCode::Enter.into())?;
    Ok((directory, app, observed))
}

fn advance(app: &mut App, now: &mut Instant, quarters: usize) -> Result<usize> {
    let mut marked = 0;
    for _ in 0..quarters {
        *now += Duration::from_millis(250);
        marked += usize::from(app.poll_auto_read(*now)?);
    }
    Ok(marked)
}

#[test]
fn entering_does_not_mark_and_one_screen_marks_only_after_active_time()
-> Result<()> {
    let (_directory, mut app, observed) = application("A short article.")?;
    let target = app.current().context("Target")?.id;
    assert!(observed.article(target)?.unread);
    let mut now = Instant::now();
    assert!(!app.poll_auto_read(now)?);
    assert_eq!(advance(&mut app, &mut now, 39)?, 0);
    assert!(observed.article(target)?.unread);
    assert_eq!(advance(&mut app, &mut now, 1)?, 1);
    assert!(!observed.article(target)?.unread);
    assert_eq!(app.current().map(|article| article.id), Some(target));
    assert_eq!(advance(&mut app, &mut now, 80)?, 0);
    Ok(())
}

#[test]
fn a_jump_to_the_end_does_not_count_hidden_text_as_seen() -> Result<()> {
    let (_directory, mut app, observed) = application(
        &"A readable paragraph with enough words.\n\n".repeat(200),
    )?;
    let mut now = Instant::now();
    app.poll_auto_read(now)?;
    app.key(KeyCode::Char('G').into())?;
    assert_eq!(advance(&mut app, &mut now, 60)?, 0);
    assert_eq!(observed.view_count(&View::Inbox)?, 1);
    let mut marked = false;
    for scroll in (0..app.reader_lines.len()).step_by(app.reader_height.max(1))
    {
        app.scroll = scroll;
        marked |= app.poll_auto_read(now)?;
    }
    assert!(marked);
    assert_eq!(observed.view_count(&View::Inbox)?, 0);
    Ok(())
}

#[test]
fn other_panes_unfocused_windows_and_dialogs_pause_active_time() -> Result<()> {
    let (_directory, mut app, observed) = application("Short text.")?;
    let mut now = Instant::now();
    app.poll_auto_read(now)?;
    advance(&mut app, &mut now, 20)?;
    app.focus = Focus::Articles;
    assert_eq!(advance(&mut app, &mut now, 80)?, 0);
    app.focus = Focus::Reader;
    let mut events = InputBatch::new(vec![Event::FocusLost]);
    events.dispatch_next(&mut app)?;
    assert_eq!(advance(&mut app, &mut now, 80)?, 0);
    let mut events = InputBatch::new(vec![Event::FocusGained]);
    events.dispatch_next(&mut app)?;
    app.key(KeyCode::Char('?').into())?;
    assert_eq!(advance(&mut app, &mut now, 80)?, 0);
    app.key(KeyCode::Esc.into())?;
    app.poll_auto_read(now)?;
    assert_eq!(advance(&mut app, &mut now, 20)?, 1);
    assert_eq!(observed.view_count(&View::Read)?, 1);
    Ok(())
}

#[test]
fn explicit_unread_suppresses_automatic_marking_for_the_current_article()
-> Result<()> {
    let (_directory, mut app, observed) = application("Short text.")?;
    app.key(KeyCode::Char(':').into())?;
    app.paste("mark-unread")?;
    app.key(KeyCode::Enter.into())?;
    let mut now = Instant::now();
    app.poll_auto_read(now)?;
    assert_eq!(advance(&mut app, &mut now, 100)?, 0);
    assert_eq!(observed.view_count(&View::Inbox)?, 1);
    Ok(())
}

#[test]
fn thresholds_round_trip_and_reject_invalid_configuration() -> Result<()> {
    let source = "[ui.auto_read]\nenabled=false\nmin_active_seconds=25\nmin_coverage_percent=80\n";
    let document = config::decode(source)?;
    assert!(!document.ui.auto_read.enabled);
    assert_eq!(document.ui.auto_read.min_active_seconds, 25);
    assert_eq!(config::decode(&config::encode(&document)?)?, document);
    for field in [
        "min_active_seconds=0",
        "min_active_seconds=3601",
        "min_coverage_percent=0",
        "min_coverage_percent=101",
    ] {
        assert!(config::decode(&format!("[ui.auto_read]\n{field}")).is_err());
    }
    Ok(())
}

#[test]
fn disabled_policy_keeps_articles_unread_until_explicitly_marked() -> Result<()>
{
    let (_directory, mut app, observed) = application("Short text.")?;
    app.key(KeyCode::Char(':').into())?;
    app.paste("auto-read off")?;
    app.key(KeyCode::Enter.into())?;
    assert!(!app.workspace.auto_read.enabled);
    let mut now = Instant::now();
    assert_eq!(advance(&mut app, &mut now, 100)?, 0);
    assert_eq!(observed.view_count(&View::Inbox)?, 1);
    app.key(KeyCode::Char('m').into())?;
    assert_eq!(observed.view_count(&View::Read)?, 1);
    Ok(())
}

#[test]
fn horizontally_hidden_code_does_not_count_toward_displayed_coverage()
-> Result<()> {
    let body = format!("```text\n{}\n```", "abcdefghij".repeat(200));
    let (_directory, mut app, observed) = application(&body)?;
    app.key(KeyCode::Char('$').into())?;
    app.horizontal = 0;
    let mut now = Instant::now();
    app.poll_auto_read(now)?;
    assert_eq!(advance(&mut app, &mut now, 60)?, 0);
    assert_eq!(observed.view_count(&View::Inbox)?, 1);
    let mut marked = false;
    for horizontal in (0..2000).step_by(app.reader_width.max(1)) {
        app.horizontal = horizontal;
        marked |= app.poll_auto_read(now)?;
    }
    assert!(marked);
    assert_eq!(observed.view_count(&View::Read)?, 1);
    Ok(())
}

#[test]
fn failed_automatic_save_reports_once_and_never_creates_a_retry_loop()
-> Result<()> {
    let (directory, mut app, observed) = application("Short text.")?;
    let injector =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    injector.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_read_state.sql"
    ))?;
    let mut now = Instant::now();
    app.poll_auto_read(now)?;
    assert_eq!(advance(&mut app, &mut now, 39)?, 0);
    let error = advance(&mut app, &mut now, 1)
        .err()
        .context("Save accepted")?;
    assert!(format!("{error:#}").contains("rejects writes"));
    assert_eq!(advance(&mut app, &mut now, 100)?, 0);
    assert_eq!(observed.view_count(&View::Inbox)?, 1);
    Ok(())
}
