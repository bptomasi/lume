//! Actual key dispatch and ordered background recommendation writes.

mod common;
use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    model::{RecommendationFeedback, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend};
use std::time::{Duration, Instant};

fn fixture() -> Result<(tempfile::TempDir, Store, lume::model::Feed)> {
    let (directory, store, feed) = common::seeded_store()?;
    let mut profile = store.recommendation_profile()?;
    profile.interests.push(lume::model::Interest {
        name: "Caching".into(),
        weight: 8,
        query: "cache".into(),
    });
    store.save_recommendation_profile(&profile)?;
    Ok((directory, store, feed))
}

fn command(app: &mut App, text: &str) -> Result<()> {
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste(text)?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    Ok(())
}

fn settle(app: &mut App) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        app.poll_recommendations()?;
        app.poll_navigation()?;
        app.poll_note_workspace()?;
        if !app.recommendations_pending()
            && !app.navigation_loading
            && !app.note_catalog_pending()
        {
            return Ok(());
        }
        anyhow::ensure!(
            Instant::now() < deadline,
            "Recommendation worker did not settle"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
}

#[test]
fn interests_feedback_why_and_palette_are_available_through_actual_commands()
-> Result<()> {
    let (directory, store, _) = fixture()?;
    let reader = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    command(&mut app, "interest My reading | 9 | rust OR sqlite")?;
    settle(&mut app)?;
    assert!(
        reader
            .recommendation_profile()?
            .interests
            .iter()
            .any(|i| i.name == "My reading")
    );
    command(&mut app, "recommendations")?;
    settle(&mut app)?;
    assert_eq!(app.view, View::Recommended);
    assert!(app.page_total > 0);
    let article = app.current().context("No recommended article")?.id;
    command(&mut app, "recommend-why")?;
    let Mode::Document(document) = &app.mode else {
        anyhow::bail!("Missing explanation");
    };
    assert!(
        document
            .lines
            .iter()
            .any(|line| line.contains("Matched interests"))
    );
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    command(&mut app, "recommend-feedback useful")?;
    settle(&mut app)?;
    assert_eq!(
        reader.recommendation_feedback(article)?,
        Some(RecommendationFeedback::Useful)
    );

    Ok(())
}

#[test]
fn recommendation_commands_cannot_rate_a_hidden_notebook_article() -> Result<()>
{
    let (directory, store, _) = fixture()?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    settle(&mut app)?;
    command(&mut app, "notes")?;
    for text in ["recommend-feedback useful", "recommend-why", "similar"] {
        assert!(command(&mut app, text).is_err(), "{text}");
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    }
    command(&mut app, "recommendations")?;
    assert!(app.notebook.is_none());
    Ok(())
}

#[test]
fn admitted_feedback_survives_navigation_and_shutdown_under_a_real_sqlite_lock()
-> Result<()> {
    let (directory, store, _) = fixture()?;
    let path = directory.path().join("library.sqlite3");
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    settle(&mut app)?;
    let article = app.current().context("No article")?.id;
    let lock = rusqlite::Connection::open(&path)?;
    lock.execute_batch("BEGIN IMMEDIATE")?;
    command(&mut app, "recommend-feedback useful")?;
    assert!(app.recommendations_pending());
    // Input remains admissible while the writer is provably unable to commit.
    app.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))?;
    assert!(matches!(app.mode, Mode::Leader));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    app.set_view(View::Read)?;
    lock.execute_batch("ROLLBACK")?;
    app.end_session()?;
    assert_eq!(
        Store::open(&path)?.recommendation_feedback(article)?,
        Some(RecommendationFeedback::Useful)
    );
    Ok(())
}

#[test]
fn cached_refresh_preserves_selected_identity_and_reader_layout_across_sizes()
-> Result<()> {
    let (directory, store, _) = fixture()?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(
        &directory.path().join("library.sqlite3"),
    )?;
    settle(&mut app)?;
    app.key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('R'), KeyModifiers::NONE))?;
    settle(&mut app)?;
    let selected = app.current().context("Missing recommendation")?.id;
    app.focus = Focus::Reader;
    let body = app.body.clone();
    command(&mut app, "recommend-feedback useful")?;
    settle(&mut app)?;
    assert_eq!(app.current().context("Selection lost")?.id, selected);
    assert_eq!(app.body, body);
    for (width, height) in [(1, 1), (40, 18), (80, 28), (144, 44)] {
        let mut terminal = Terminal::new(TestBackend::new(width, height))?;
        app.resize_workspace(ratatui::layout::Rect::new(0, 0, width, height));
        terminal.draw(|frame| ui::draw(frame, &app))?;
    }
    command(&mut app, "help recommendations")?;
    assert!(matches!(app.mode, Mode::Help));
    Ok(())
}

#[test]
fn queued_settings_compose_against_the_latest_durable_profile() -> Result<()> {
    let (directory, store, _) = fixture()?;
    let path = directory.path().join("library.sqlite3");
    let reader = Store::open(&path)?;
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    command(&mut app, "recommend-settings minutes 20")?;
    command(&mut app, "recommend-settings read on")?;
    settle(&mut app)?;
    let profile = reader.recommendation_profile()?;
    assert_eq!(profile.minutes, Some(20));
    assert!(profile.include_read);
    Ok(())
}

#[test]
fn queue_overload_rejects_new_edits_without_replacing_admitted_preferences()
-> Result<()> {
    let (directory, store, _) = fixture()?;
    let path = directory.path().join("library.sqlite3");
    let initial = store.recommendation_profile()?.interests.len();
    let mut app = App::new(store, false)?;
    app.enable_background_navigation(&path)?;
    settle(&mut app)?;
    let lock = rusqlite::Connection::open(&path)?;
    lock.execute_batch("BEGIN IMMEDIATE")?;
    for index in 0..8 {
        command(&mut app, &format!("interest Subject {index} | 3 | cache"))?;
    }
    assert!(command(&mut app, "interest Rejected | 3 | cache").is_err());
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    lock.execute_batch("ROLLBACK")?;
    app.end_session()?;
    let profile = Store::open(&path)?.recommendation_profile()?;
    assert_eq!(profile.interests.len(), initial + 8);
    assert!(
        !profile
            .interests
            .iter()
            .any(|interest| interest.name == "Rejected")
    );
    Ok(())
}
