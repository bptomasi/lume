//! Modal workflows exercise cursor edits, source capture and session recovery.

mod common;

use anyhow::{Result, ensure};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Focus, Mode},
    model::View,
    store::Store,
};

fn key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> Result<()> {
    app.key(KeyEvent::new(code, modifiers))?;
    Ok(())
}

fn text(app: &mut App, value: &str) -> Result<()> {
    for character in value.chars() {
        key(app, KeyCode::Char(character), KeyModifiers::NONE)?;
    }
    Ok(())
}

fn save(app: &mut App) -> Result<()> {
    key(app, KeyCode::Char('s'), KeyModifiers::CONTROL)
}

#[test]
fn cursor_edits_and_undo_preserve_unicode() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    text(&mut app, "nação")?;
    key(&mut app, KeyCode::Home, KeyModifiers::NONE)?;
    text(&mut app, "🦀 ")?;
    key(&mut app, KeyCode::Char('z'), KeyModifiers::CONTROL)?;
    key(&mut app, KeyCode::Char('r'), KeyModifiers::CONTROL)?;
    save(&mut app)?;
    assert_eq!(app.comments[0].body, "🦀 ação");
    Ok(())
}

#[test]
fn visual_selection_saves_quote_and_notebook_retains_context() -> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE)?;
    text(&mut app, "vjnA useful passage")?;
    ensure!(
        matches!(app.mode, Mode::Input(_)),
        "Highlight should open a note editor"
    );
    save(&mut app)?;
    assert!(!app.comments[0].quote.is_empty());
    assert_eq!(app.comments[0].body, "A useful passage");

    text(&mut app, " n")?;

    assert_eq!(app.view, View::Annotated);
    assert_eq!(app.focus, Focus::Reader);
    assert!(
        app.reader_lines
            .iter()
            .any(|line| line.contains("A useful passage"))
    );
    assert!(app.reader_lines.iter().any(|line| line.starts_with('│')));
    Ok(())
}

#[test]
fn command_palette_keeps_opened_modal_and_list_picker_commits_multiple()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let first = store.create_collection("Research")?;
    let second = store.create_collection("Weekend")?;
    let mut app = App::new(store, false)?;
    text(&mut app, ":palette")?;
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE)?;
    assert!(matches!(app.mode, Mode::Picker(_)));
    text(&mut app, "organize")?;
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE)?;
    text(&mut app, " ")?;
    key(&mut app, KeyCode::Down, KeyModifiers::NONE)?;
    text(&mut app, " ")?;
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE)?;

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let id = app.current().map_or(0, |article| article.id);
    assert_eq!(store.article_memberships(id)?, [first, second].into());
    text(&mut app, "u")?;
    assert!(store.article_memberships(id)?.is_empty());
    Ok(())
}

#[test]
fn smart_form_saves_typed_criteria_and_layout_survives_restart() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    text(&mut app, " s")?;
    text(&mut app, "Careful reading")?;
    key(&mut app, KeyCode::Tab, KeyModifiers::NONE)?;
    text(&mut app, "measure")?;
    save(&mut app)?;
    assert_eq!(app.library.smart_lists[0].filter.text, "measure");
    text(&mut app, "z")?;
    app.save_session()?;
    let id = app.current().map(|article| article.id);
    drop(app);

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let restored = App::new(store, false)?;

    assert!(restored.workspace.zen);
    assert_eq!(restored.current().map(|article| article.id), id);
    Ok(())
}

#[test]
fn external_editor_failure_retains_draft_and_success_reads_private_file()
-> Result<()> {
    assert!(lume::external_editor::edit("Retain me", "/bin/false").is_err());
    let edited = lume::external_editor::edit(
        "Original draft",
        "python3 -c 'import pathlib,sys; pathlib.Path(sys.argv[1]).write_text(\"Edited thought\")'",
    )?;
    assert_eq!(edited, "Edited thought");
    Ok(())
}

#[test]
fn navigation_crosses_pages_and_restores_destination_with_old_selection()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let articles = (0..450)
        .map(|index| lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: format!("page-{index}"),
            title: format!("Page {index}"),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000 + index,
            body: "Cached article".into(),
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &articles)?;
    let mut app = App::new(store, false)?;
    key(&mut app, KeyCode::Char('G'), KeyModifiers::NONE)?;
    assert_eq!(app.page_offset, 400);
    assert_eq!(app.selected, 50);
    let id = app.current().map(|article| article.id);
    app.save_session()?;
    drop(app);

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let app = App::new(store, false)?;

    assert_eq!(app.view, View::Inbox);
    assert_eq!(app.page_offset, 400);
    assert_eq!(app.current().map(|article| article.id), id);
    Ok(())
}

#[test]
fn reading_anchor_survives_restart_at_a_different_width() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.resize_reader(12, 2);
    key(&mut app, KeyCode::Enter, KeyModifiers::NONE)?;
    text(&mut app, "jjj")?;
    let anchor = app.line_ranges[app.scroll].start;
    ensure!(anchor > 0, "Scenario must move past the first source byte");
    app.save_session()?;
    drop(app);

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.resize_reader(18, 2);

    assert!(app.line_ranges[app.scroll].contains(&anchor));
    assert_eq!(app.reader_lines[app.scroll], "Keep one copy &");
    assert!(app.current().is_some());
    Ok(())
}
