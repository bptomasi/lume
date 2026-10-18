//! Partial feed content stays honest, actionable and separate from quotations.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus},
    content::{self, BodyKind},
    model::{IncomingArticle, View},
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, layout::Rect};

const DISCUSSION: &str = "[Comments](<https://community.example/discussion>)";

fn reading(body: &str, url: &str) -> Result<(tempfile::TempDir, App)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    store.ingest(
        &feed,
        "Partial journal",
        &[IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "partial-entry".into(),
            title: "An article discovered through a community".into(),
            url: url.into(),
            author: "Original test fixture".into(),
            published: 2_000_000_000,
            body: body.into(),
        }],
    )?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    Ok((directory, app))
}

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

#[test]
fn availability_distinguishes_references_without_guessing_from_length() {
    for body in [
        "A short summary.",
        "[Context](https://example.org) explains the failure.",
        "`return 0;`",
        "<T>",
        "~~~rust\nlet ready = true;\n~~~",
    ] {
        assert_eq!(content::body_kind(body), BodyKind::Text, "{body}");
    }
    for body in [
        DISCUSSION,
        "[**Comments**](https://example.org)\n\n[Source](https://example.net)",
        "![Architecture diagram](https://example.org/diagram.png)",
        "[![Video](https://example.org/poster.png)](https://example.org/watch)",
    ] {
        assert_eq!(
            content::body_kind(body),
            BodyKind::ReferencesOnly,
            "{body}"
        );
    }
    for body in ["", " \n\t", "---", content::MISSING_BODY] {
        assert_eq!(content::body_kind(body), BodyKind::Missing, "{body}");
    }
}

#[test]
fn cached_discussion_only_article_explains_the_feed_and_opens_the_author()
-> Result<()> {
    let url = "https://author.example/article";
    let (directory, mut app) = reading(DISCUSSION, url)?;
    let id = app.current().context("Missing article")?.id;
    assert_eq!(app.body, DISCUSSION);
    assert_eq!(app.body_kind, BodyKind::ReferencesOnly);
    assert_eq!(app.reading_minutes, 0);
    assert!(app.reader_text.starts_with("Comments\n"));
    assert!(app.reader_text.contains("without separate article text"));
    assert!(app.reader_text.contains("Open the original article"));
    assert_eq!(
        press(&mut app, KeyCode::Char('o'))?,
        Action::Open(url.into())
    );
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(store.body(id)?, DISCUSSION);
    assert!(store.comments(id)?.is_empty());
    Ok(())
}

#[test]
fn legacy_missing_body_has_no_fake_reading_time_or_duplicated_placeholder()
-> Result<()> {
    let (_directory, mut app) = reading(content::MISSING_BODY, "")?;
    assert_eq!(app.body_kind, BodyKind::Missing);
    assert_eq!(app.reading_minutes, 0);
    assert!(app.reader_text.contains("no readable article text"));
    assert!(app.reader_text.contains("No original article URL"));
    assert!(!app.reader_text.contains(content::MISSING_BODY));
    assert!(!app.reader_text.contains("Open the original article"));
    assert_eq!(press(&mut app, KeyCode::Char('o'))?, Action::None);
    Ok(())
}

#[test]
fn generated_feed_notice_cannot_be_saved_as_a_publisher_highlight() -> Result<()>
{
    let (directory, mut app) = reading(DISCUSSION, "https://example.org")?;
    let id = app.current().context("Missing article")?.id;
    app.reader_cursor = app
        .reader_lines
        .iter()
        .position(|line| line.contains("This feed supplied"))
        .context("Missing availability notice")?;
    app.reader_column = 0;
    press(&mut app, KeyCode::Char('v'))?;
    let error = press(&mut app, KeyCode::Char('s'))
        .err()
        .context("Generated notice was accepted as article text")?;
    assert_eq!(
        error.to_string(),
        "Select article text to capture a highlight"
    );
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    assert!(store.comments(id)?.is_empty());
    assert_eq!(store.body(id)?, DISCUSSION);
    Ok(())
}

#[test]
fn notice_reflows_in_panels_and_fullscreen_without_affecting_normal_articles()
-> Result<()> {
    let (_directory, mut app) =
        reading(DISCUSSION, "https://author.example/article")?;
    for fullscreen in [false, true] {
        app.workspace.fullscreen = fullscreen;
        for (width, height) in [(40, 20), (80, 24), (144, 44)] {
            let mut terminal = Terminal::new(TestBackend::new(width, height))?;
            let reader = ui::workspace_areas(
                Rect::new(0, 0, width, height),
                app.focus,
                &app.workspace,
            )
            .reader
            .context("Missing reader")?;
            let body = ui::workspace_reader_content(reader, &app.workspace);
            app.resize_reader(body.width.into(), body.height.into());
            terminal.draw(|frame| ui::draw(frame, &app))?;
            let screen = terminal
                .backend()
                .buffer()
                .content
                .iter()
                .map(|cell| cell.symbol())
                .collect::<String>();
            assert!(screen.contains("Comments"));
            assert!(screen.contains("links / media only"));
            assert!(!screen.contains("1 min"));
        }
    }
    app.focus = Focus::Articles;
    press(&mut app, KeyCode::Char('j'))?;
    assert_eq!(app.body_kind, BodyKind::Text);
    assert!(app.reading_minutes > 0);
    assert!(!app.reader_text.contains("This feed supplied"));
    Ok(())
}

#[test]
fn notebook_keeps_private_notes_without_showing_feed_availability() -> Result<()>
{
    let (directory, app) = reading(DISCUSSION, "https://example.org")?;
    let id = app.current().context("Missing article")?.id;
    drop(app);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    store.save_comment(id, None, "Investigate the original source.")?;
    let mut workspace = store.workspace()?;
    workspace.view = View::Annotated;
    store.save_workspace(&workspace)?;
    let app = App::new(store, false)?;
    assert!(app.reader_text.contains("Investigate the original source."));
    assert!(!app.reader_text.contains("This feed supplied"));
    Ok(())
}
