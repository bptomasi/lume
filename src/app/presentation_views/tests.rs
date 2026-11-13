//! Cached factories must remain cheap borrows even with an unfinished editor.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

use super::*;
use crate::{model::IncomingArticle, store::Store, ui};

fn application() -> Result<(tempfile::TempDir, App)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let feed = store.subscribe("https://journal.example/rss")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|candidate| candidate.id == feed)
        .context("Subscribed feed missing")?;
    store.ingest(
        &feed,
        "Journal",
        &[IncomingArticle {
            external_id: "presentation".into(),
            title: "Cached presentation source".into(),
            url: "https://journal.example/article".into(),
            body: "Alpha bravo charlie delta.\n\nSecond paragraph.".into(),
            author: String::new(),
            published: 0,
            body_origin: crate::model::FeedOrigin::Feed,
        }],
    )?;
    let mut app = App::new(store, false)?;
    app.resize_workspace(Rect::new(0, 0, 160, 44));
    Ok((directory, app))
}

#[test]
fn factory_borrows_source_rows_and_editor_without_touching_the_label_cache()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste("feed https://unfinished.example/")?;
    let labels = app.headlines.borrow_mut();
    let view = app.presentation();
    assert!(std::ptr::eq(view.overlays.mode, app.mode()));
    assert!(std::ptr::eq(view.reader.reader.body, app.reader().body));
    assert!(std::ptr::eq(
        view.reader.reader.reader_styled,
        app.reader().reader_styled,
    ));
    assert!(std::ptr::eq(
        view.navigation.navigation.visible,
        app.navigation().visible,
    ));
    assert!(std::ptr::eq(
        view.navigation.navigation.navigation,
        app.navigation().navigation,
    ));
    let Mode::Input(input) = view.overlays.mode else {
        anyhow::bail!("Command draft was replaced");
    };
    assert_eq!(input.text, "feed https://unfinished.example/");
    assert!(!view.reader.reader.body.is_empty());
    drop(labels);
    Ok(())
}

#[test]
fn repeated_cached_frames_preserve_selection_source_and_unfinished_draft()
-> Result<()> {
    let (_directory, mut app) = application()?;
    app.activate_pane(Pane::Reader);
    for key in ['v', 'l', ':'] {
        app.key(KeyEvent::new(KeyCode::Char(key), KeyModifiers::NONE))?;
    }
    app.paste("find unfinished")?;
    let selection =
        app.reader().selected_range().context("Selection missing")?;
    let source = app.reader().reader_text.as_ptr();
    let view = app.presentation();
    let mut terminal = Terminal::new(TestBackend::new(160, 44))?;
    terminal.draw(|frame| ui::draw(frame, &view))?;
    let first = terminal.backend().buffer().clone();
    terminal.draw(|frame| ui::draw(frame, &view))?;
    assert_eq!(terminal.backend().buffer(), &first);
    assert_eq!(app.reader().selected_range(), Some(selection));
    assert_eq!(app.reader().reader_text.as_ptr(), source);
    let Mode::Input(input) = app.mode() else {
        anyhow::bail!("Command draft was replaced while drawing");
    };
    assert_eq!(input.text, "find unfinished");
    Ok(())
}
