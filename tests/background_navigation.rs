//! Real WAL storage and worker boundaries qualify replaceable interactive reads.

mod common;

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::App,
    model::{Pane, View},
    store::Store,
};
use ratatui::layout::Rect;

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while app.navigation_loading
        || app.reader_pending()
        || app.catalog_pending()
        || app.notes_pending()
    {
        app.poll_note_workspace()?;
        app.poll_navigation()?;
        app.poll_reader();
        app.resize_workspace(Rect::new(0, 0, 144, 44));
        anyhow::ensure!(Instant::now() < deadline, "Navigation did not settle");
        std::thread::yield_now();
    }
    Ok(())
}

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

#[test]
fn newest_destination_wins_and_readable_content_matches_its_identity()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.enable_background_navigation(&path)?;
    for _ in 0..100 {
        app.set_view(View::All)?;
        app.set_view(View::Starred)?;
    }
    app.set_view(View::All)?;
    settle(&mut app)?;
    let id = app.current().context("Article missing")?.id;
    let expected = Store::open(&path)?.body(id)?;
    assert_eq!(app.body, expected);
    assert!(!app.reader_text.is_empty());
    assert_eq!(app.view, View::All);
    app.end_session()?;
    assert!(!app.session_pending());
    Ok(())
}

#[test]
fn locked_writer_does_not_block_navigation_or_layout_admission() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.enable_background_navigation(&path)?;
    let lock = rusqlite::Connection::open(&path)?;
    lock.execute_batch("BEGIN IMMEDIATE")?;

    app.set_view(View::All)?;
    command(&mut app, "workspace load Study")?;
    settle(&mut app)?;
    assert!(app.current().is_some());
    assert!(app.session_pending());
    assert!(app.workspace.layout.is_some());

    lock.execute_batch("ROLLBACK")?;
    app.end_session()?;
    assert!(!app.session_pending());
    Ok(())
}

#[test]
fn completed_layout_preserves_a_live_selection() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    app.focus = Pane::Reader;
    app.key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))?;
    let selected =
        app.reader_text[app.selected_range().context("No passage")?].to_owned();
    let anchor = app.selection;
    app.resize_reader(50, 22);
    settle(&mut app)?;
    assert_eq!(app.selection, anchor);
    assert_eq!(
        &app.reader_text[app.selected_range().context("Selection lost")?],
        selected
    );
    app.end_session()?;
    Ok(())
}

#[test]
fn explicit_source_jumps_cancel_stale_reads_and_keep_source_bound_tools()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    let full = "# Original article\n\nA café and a systems design question.\n";
    store.cache_document(
        &lume::model::ArticleTarget {
            article: article.id,
            url: article.url,
        },
        &lume::model::ExtractedDocument {
            final_url: "https://journal.example/full".into(),
            body: full.into(),
        },
    )?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(View::Starred)?;
    app.jump_article(article.id)?;
    command(&mut app, "content feed")?;
    assert_eq!(app.content_source, lume::model::ContentSource::Feed);
    command(&mut app, "content full")?;
    assert_eq!(app.body, full);
    assert_eq!(app.content_source, lume::model::ContentSource::Full);
    app.focus = Pane::Reader;
    app.key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('$'), KeyModifiers::NONE))?;
    let selected = app.reader_text
        [app.selected_range().context("Missing selection")?]
    .to_owned();
    settle(&mut app)?;
    assert_eq!(app.body, full);
    assert_eq!(
        &app.reader_text[app.selected_range().context("Lost selection")?],
        selected
    );
    app.end_session()?;
    Ok(())
}

#[test]
fn journal_write_errors_stay_visible_and_shutdown_does_not_report_success()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    let injector = rusqlite::Connection::open(&path)?;
    injector.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_layout_history.sql"
    ))?;
    command(&mut app, "workspace load Study")?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while !app.session_failed() {
        anyhow::ensure!(
            Instant::now() < deadline,
            "Write failure was not reported"
        );
        std::thread::yield_now();
    }
    assert!(app.flush_session().is_err());
    assert!(app.end_session().is_err());
    let saved = Store::open(&path)?.workspace()?;
    assert!(saved.layout.is_none());
    assert!(!saved.show_annotations);
    Ok(())
}

#[test]
fn deferred_layout_retains_saved_source_anchor_when_private_notes_are_visible()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let body = (0..120).map(|index| format!("Paragraph {index}: a café, a distributed system and a precise reading anchor.\n\n")).collect::<String>();
    store.ingest(
        &feed,
        "Anchor fixture",
        &[lume::model::IncomingArticle {
            external_id: "anchor-fixture".into(),
            title: "Long source anchor fixture".into(),
            url: "https://journal.example/anchor".into(),
            author: "Fixture".into(),
            published: 9_000_000_000,
            body,
            body_origin: lume::model::FeedOrigin::Feed,
        }],
    )?;
    let id = store.library()?.articles.remove(0).id;
    store.save_progress(id, 1500)?;
    store.save_comment(id, None, "A private note below the article.")?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 144, 44));
    assert!(app.scroll > 0);
    let anchor = app.line_ranges[app.scroll].start;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;

    app.set_view(View::All)?;
    settle(&mut app)?;

    assert_eq!(app.current().context("Selection lost")?.id, id);
    assert!(
        app.scroll > 0,
        "Background layout lost the saved source position"
    );
    assert_eq!(app.line_ranges[app.scroll].start, anchor);
    assert!(
        app.reader_text
            .contains("A private note below the article.")
    );
    app.end_session()?;
    Ok(())
}

#[test]
fn additional_article_steps_replace_the_selection_of_a_page_still_loading()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let articles = (0..6)
        .map(|index| lume::model::IncomingArticle {
            external_id: format!("pending-page-{index}"),
            title: format!("Row {index}"),
            url: format!("https://journal.example/page/{index}"),
            author: "Fixture".into(),
            published: 3_000_000_000 + index,
            body: format!("Content for row {index}."),
            body_origin: lume::model::FeedOrigin::Feed,
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, "Page fixture", &articles)?;
    let mut app = App::new(store, false)?;
    app.page_size = 2;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    app.focus = Pane::Articles;

    for _ in 0..3 {
        app.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))?;
    }
    settle(&mut app)?;

    assert_eq!(app.current().context("Selection missing")?.title, "Row 2");
    assert_eq!(app.body, "Content for row 2.");
    assert_eq!(app.page_offset, 2);
    assert_eq!(app.selected, 1);
    app.end_session()?;
    Ok(())
}

#[test]
fn opening_unread_admits_write_without_waiting_for_a_locked_database()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let observer = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    let id = app.current().context("Article")?.id;
    let lock = rusqlite::Connection::open(&path)?;
    lock.execute_batch("BEGIN IMMEDIATE")?;

    app.focus = Pane::Articles;
    let start = Instant::now();
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert!(start.elapsed() < Duration::from_millis(200));
    assert_eq!(app.focus, Pane::Reader);
    assert!(!app.current().context("Selection")?.unread);
    assert!(observer.article(id)?.unread);
    assert!(app.notes_pending());

    lock.execute_batch("ROLLBACK")?;
    settle(&mut app)?;
    assert!(!observer.article(id)?.unread);
    let count = app
        .navigation
        .iter()
        .find(|item| item.view == View::Read)
        .context("Read folder")?
        .count;
    assert_eq!(count, observer.view_count(&View::Read)?);
    app.end_session()?;
    Ok(())
}

#[test]
fn manual_unread_and_undo_follow_the_admitted_automatic_read() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let observer = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    let id = app.current().context("Article")?.id;
    let lock = rusqlite::Connection::open(&path)?;
    lock.execute_batch("BEGIN IMMEDIATE")?;
    app.focus = Pane::Articles;

    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    command(&mut app, "mark-unread")?;
    app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE))?;
    lock.execute_batch("ROLLBACK")?;
    settle(&mut app)?;

    assert!(
        !observer.article(id)?.unread,
        "Undo must revert manual unread"
    );
    assert!(!app.current().context("Selection")?.unread);
    app.end_session()?;
    Ok(())
}

#[test]
fn catalog_refresh_preserves_selection_and_a_live_publisher_quotation()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let mut publisher = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    let id = app.current().context("Article")?.id;
    app.focus = Pane::Reader;
    app.key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('l'), KeyModifiers::NONE))?;
    let anchor = app.selection;
    let cursor = (app.reader_cursor, app.reader_column);
    let text = app.reader_text.clone();
    publisher.ingest(&feed, "Refreshed source", &[new_article()])?;

    app.reload()?;
    assert!(app.catalog_pending());
    assert_eq!(app.reader_text, text);
    assert_eq!(app.current().context("Selection")?.id, id);
    app.poll_navigation()?;
    assert_eq!(app.selection, anchor);
    app.selection = None;
    settle(&mut app)?;

    assert_eq!(app.current().context("Selection lost")?.id, id);
    assert_eq!(app.reader_text, text);
    assert_eq!((app.reader_cursor, app.reader_column), cursor);
    assert_eq!(app.page_total, 2);
    app.end_session()?;
    Ok(())
}

fn new_article() -> lume::model::IncomingArticle {
    lume::model::IncomingArticle {
        external_id: "new-refresh".into(),
        title: "New collection arrival".into(),
        url: "https://journal.example/new".into(),
        author: "Publisher".into(),
        published: 9_000_000_000,
        body: "New publisher text.".into(),
        body_origin: lume::model::FeedOrigin::Feed,
    }
}

#[test]
fn retained_preview_keeps_its_title_and_rejects_source_bound_actions()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    store.ingest(&feed, "More articles", &[new_article()])?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    let title = app.current().context("Article")?.title.clone();
    let body = app.reader_text.clone();
    app.focus = Pane::Articles;

    app.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))?;
    assert!(app.reader_stale());
    assert_eq!(app.reader_text, body);
    assert_eq!(
        app.displayed_article().context("Retained title")?.title,
        title
    );
    app.focus = Pane::Reader;
    app.key(KeyEvent::new(KeyCode::Char('G'), KeyModifiers::NONE))?;
    assert_eq!(app.reader_cursor, app.reader_lines.len() - 1);
    app.key(KeyEvent::new(KeyCode::Char('v'), KeyModifiers::NONE))?;
    assert!(app.selection.is_none());
    assert!(command(&mut app, "note").is_err());
    app.mode = lume::app::Mode::Normal;
    settle(&mut app)?;

    assert!(!app.reader_stale());
    assert_ne!(app.reader_text, body);
    app.end_session()?;
    Ok(())
}

#[test]
fn newer_destination_replaces_a_pending_catalog_refresh() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.reload()?;
    app.set_view(View::Starred)?;
    app.set_view(View::All)?;
    settle(&mut app)?;

    assert_eq!(app.view, View::All);
    assert_eq!(app.page_total, 1);
    assert!(app.current().is_some());
    assert!(!app.reader_text.is_empty());
    app.end_session()?;
    Ok(())
}

#[test]
fn failed_read_save_reports_the_failure_and_reconciles_optimistic_flags()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("library.sqlite3");
    let observer = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    let id = app.current().context("Article")?.id;
    let injector = rusqlite::Connection::open(&path)?;
    injector.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_read_state.sql"
    ))?;
    app.focus = Pane::Articles;

    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let error = loop {
        if let Err(error) = app.poll_note_tasks() {
            break error;
        }
        anyhow::ensure!(Instant::now() < deadline, "Write failure was lost");
        std::thread::yield_now();
    };
    assert!(format!("{error:#}").contains("rejects writes"));
    settle(&mut app)?;

    assert!(observer.article(id)?.unread);
    assert!(app.current().context("Selection")?.unread);
    app.end_session()?;
    Ok(())
}

#[test]
fn page_navigation_publishes_full_content_only_after_its_projection_is_ready()
-> Result<()> {
    let (directory, store, full) = paged_full_source()?;
    let mut app = App::new(store, false)?;
    app.page_size = 1;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    app.set_view(View::All)?;
    settle(&mut app)?;
    let previous = app.reader_text.clone();
    app.focus = Pane::Articles;

    app.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))?;
    let deadline = Instant::now() + Duration::from_secs(5);
    while app.navigation_loading {
        assert_eq!(app.reader_text, previous, "Retained preview was cleared");
        app.poll_reader();
        app.poll_navigation()?;
        anyhow::ensure!(
            Instant::now() < deadline,
            "Projection did not publish"
        );
        std::thread::yield_now();
    }

    assert_eq!(app.body, full);
    assert_eq!(app.content_source, lume::model::ContentSource::Full);
    assert!(!app.article_preparing());
    assert!(app.reader_text.contains("Full source content."));
    app.end_session()?;
    Ok(())
}

fn paged_full_source() -> Result<(tempfile::TempDir, Store, String)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let old = store.library()?.articles.remove(0);
    let full = format!(
        "# Cached source\n\n{}",
        "Full source content.\n\n".repeat(200)
    );
    store.cache_document(
        &lume::model::ArticleTarget {
            article: old.id,
            url: old.url,
        },
        &lume::model::ExtractedDocument {
            final_url: "https://journal.example/full".into(),
            body: full.clone(),
        },
    )?;
    store.ingest(&feed, "Page source", &[new_article()])?;
    Ok((directory, store, full))
}

#[test]
fn automatic_read_keeps_an_opened_unread_smart_list_article() -> Result<()> {
    for pending in [false, true] {
        let (_directory, mut app, observer, view) = unread_smart_reader()?;
        app.focus = Pane::Articles;
        if pending {
            app.key(KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE))?;
        }
        let id = app.current().context("Selected article")?.id;
        let expected = observer.body(id)?;

        app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
        settle(&mut app)?;

        assert_eq!(app.current().context("Retained article")?.id, id);
        assert_eq!(app.body, expected);
        assert!(!observer.article(id)?.unread);
        let count = app
            .navigation
            .iter()
            .find(|item| item.view == view)
            .context("Unread smart list")?
            .count;
        assert_eq!(count, 1);
        app.set_view(view)?;
        settle(&mut app)?;
        assert_ne!(app.current().context("Remaining unread")?.id, id);
        app.end_session()?;
    }
    Ok(())
}

fn unread_smart_reader() -> Result<(tempfile::TempDir, App, Store, View)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    store.ingest(&feed, "Unread source", &[new_article()])?;
    store.save_smart_list(
        "Unread systems",
        &lume::model::SmartFilter {
            unread: true,
            ..Default::default()
        },
    )?;
    let view = View::Smart(store.catalog()?.smart_lists[0].id);
    let path = directory.path().join("library.sqlite3");
    let observer = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    app.set_view(view.clone())?;
    settle(&mut app)?;
    Ok((directory, app, observer, view))
}
