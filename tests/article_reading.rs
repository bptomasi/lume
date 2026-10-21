//! Source switching and background completion preserve selection and drafts.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus, InputKind, Mode},
    model::{
        ArticleTarget, ContentSource, ExtractedDocument, FetchPolicy,
        IncomingArticle, View,
    },
    source,
    store::Store,
};

const DISCUSSION: &str = "[Comments](https://community.example/discussion)";

fn reading(
    body: &str,
) -> Result<(tempfile::TempDir, App, Store, ArticleTarget)> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let mut incoming =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?.articles;
    incoming[0].body = body.into();
    store.ingest(&feed, "Community journal", &incoming)?;
    let article = store.library()?.articles.remove(0);
    let target = ArticleTarget {
        article: article.id,
        url: article.url,
    };
    let writer = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Reader;
    Ok((directory, app, writer, target))
}

fn press(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    press(app, KeyCode::Char(':'))?;
    for character in text.chars() {
        press(app, KeyCode::Char(character))?;
    }
    press(app, KeyCode::Enter)
}

fn cache(store: &Store, target: &ArticleTarget, body: &str) -> Result<()> {
    assert!(store.cache_document(
        target,
        &ExtractedDocument {
            final_url: "https://author.example/canonical".into(),
            body: body.into(),
        }
    )?);
    Ok(())
}

#[test]
fn automatic_fetch_targets_only_the_opened_reader_with_missing_text()
-> Result<()> {
    let (_directory, mut app, _writer, target) = reading(DISCUSSION)?;
    let now = chrono::Utc::now().timestamp();
    assert_eq!(app.automatic_article_request(now), Some(target));
    app.focus = Focus::Articles;
    assert!(app.automatic_article_request(now).is_none());
    app.focus = Focus::Reader;
    app.view = View::Trash;
    assert!(app.automatic_article_request(now).is_none());
    let (_directory, app, _writer, _target) =
        reading("A useful short summary.")?;
    assert!(app.automatic_article_request(now).is_none());
    Ok(())
}

#[test]
fn per_source_policy_handles_summaries_and_manual_fetching_explicitly()
-> Result<()> {
    let (_directory, mut app, writer, target) = reading("A useful summary.")?;
    let now = chrono::Utc::now().timestamp();
    command(&mut app, "fetch-policy always")?;
    assert_eq!(app.automatic_article_request(now), Some(target.clone()));
    let feed = app.current().context("Missing article")?.feed_id;
    assert_eq!(writer.fetch_policy(feed)?, FetchPolicy::Always);
    command(&mut app, "fetch-policy manual")?;
    assert!(app.automatic_article_request(now).is_none());
    assert_eq!(command(&mut app, "fetch")?, Action::FetchArticle(target));
    assert!(command(&mut app, "fetch-policy guessing").is_err());
    Ok(())
}

#[test]
fn cached_full_article_reopens_offline_and_keeps_the_feed_discussion_link()
-> Result<()> {
    let (directory, app, writer, target) = reading(DISCUSSION)?;
    drop(app);
    cache(
        &writer,
        &target,
        "A full cached article with technical reasoning.",
    )?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;

    let mut reopened = App::new(store, false)?;

    assert_eq!(reopened.content_source, ContentSource::Full);
    assert!(reopened.body.contains("full cached article"));
    assert!(
        reopened
            .links
            .iter()
            .any(|(_, url)| url == "https://community.example/discussion")
    );
    assert!(
        reopened
            .automatic_article_request(chrono::Utc::now().timestamp())
            .is_none()
    );
    command(&mut reopened, "content feed")?;
    assert_eq!(reopened.content_source, ContentSource::Feed);
    assert_eq!(reopened.body, DISCUSSION);
    command(&mut reopened, "content")?;
    assert_eq!(reopened.content_source, ContentSource::Full);
    assert!(reopened.body.contains("full cached article"));
    assert_eq!(writer.body(target.article)?, DISCUSSION);
    Ok(())
}

#[test]
fn completion_waits_for_visual_selection_and_preserves_its_yank() -> Result<()>
{
    let (_directory, mut app, writer, target) = reading(DISCUSSION)?;
    app.article_fetch_started(target.clone());
    press(&mut app, KeyCode::Char('v'))?;
    cache(&writer, &target, "Full text retrieved while selecting.")?;

    app.article_fetch_finished(target, None);

    assert!(!app.apply_document_completion()?);
    assert_eq!(app.body, DISCUSSION);
    assert_eq!(
        press(&mut app, KeyCode::Char('y'))?,
        Action::Copy("C".into())
    );
    press(&mut app, KeyCode::Esc)?;
    assert!(app.apply_document_completion()?);
    assert_eq!(app.content_source, ContentSource::Full);
    assert!(app.body.contains("while selecting"));
    Ok(())
}

#[test]
fn a_feed_note_draft_keeps_its_captured_source_while_full_text_arrives()
-> Result<()> {
    let (_directory, mut app, writer, target) = reading(DISCUSSION)?;
    app.article_fetch_started(target.clone());
    press(&mut app, KeyCode::Char('v'))?;
    press(&mut app, KeyCode::Char('n'))?;
    cache(&writer, &target, "A newly retrieved article body.")?;
    app.article_fetch_finished(target.clone(), None);

    assert!(!app.apply_document_completion()?);
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Annotation draft was replaced");
    };
    let InputKind::Highlight { passage, .. } = &input.kind else {
        anyhow::bail!("Expected a captured passage");
    };
    assert_eq!(passage.source, ContentSource::Feed);
    assert_eq!(passage.quote, "C");
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    let note = writer.comments(target.article)?.remove(0);
    assert_eq!(note.quote, "C");
    assert_eq!(note.source, ContentSource::Feed);
    assert_eq!(note.source_revision, 0);
    Ok(())
}

#[test]
fn completion_for_a_previous_article_never_changes_the_new_selection()
-> Result<()> {
    let (_directory, mut app, mut writer, target) = reading(DISCUSSION)?;
    app.article_fetch_started(target.clone());
    let feed = writer.feeds()?.remove(0);
    writer.ingest(
        &feed,
        "Community journal",
        &[IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "second".into(),
            title: "The next article".into(),
            url: "https://journal.example/next".into(),
            author: String::new(),
            published: 2_000_000_000,
            body: "Second article text.".into(),
        }],
    )?;
    app.reload()?;
    app.focus = Focus::Articles;
    press(&mut app, KeyCode::Home)?;
    let selected = app.current().context("Missing new selection")?.id;
    assert_ne!(selected, target.article);
    cache(&writer, &target, "The previous article's complete text.")?;

    app.article_fetch_finished(target, None);

    assert!(!app.apply_document_completion()?);
    assert_eq!(app.current().context("Selection disappeared")?.id, selected);
    assert_eq!(app.body, "Second article text.");
    Ok(())
}

#[test]
fn choosing_feed_during_a_download_prevents_an_automatic_source_switch()
-> Result<()> {
    let (_directory, mut app, writer, target) = reading(DISCUSSION)?;
    app.article_fetch_started(target.clone());
    command(&mut app, "content feed")?;
    cache(&writer, &target, "The complete article is now available.")?;

    app.article_fetch_finished(target, None);

    app.apply_document_completion()?;
    assert_eq!(app.content_source, ContentSource::Feed);
    assert_eq!(app.body, DISCUSSION);
    assert!(
        app.automatic_article_request(chrono::Utc::now().timestamp())
            .is_none()
    );
    command(&mut app, "content full")?;
    assert_eq!(app.content_source, ContentSource::Full);
    Ok(())
}

#[test]
fn failed_cache_write_has_a_session_cooldown_even_without_a_failure_record()
-> Result<()> {
    let (_directory, mut app, _writer, target) = reading(DISCUSSION)?;
    app.article_fetch_started(target.clone());
    app.article_fetch_finished(target.clone(), Some("Database unavailable"));
    app.apply_document_completion()?;
    let now = chrono::Utc::now().timestamp();

    assert!(app.automatic_article_request(now).is_none());
    assert_eq!(
        app.automatic_article_request(now + 901),
        Some(target.clone())
    );
    assert_eq!(command(&mut app, "fetch")?, Action::FetchArticle(target));
    assert_eq!(app.body, DISCUSSION);
    Ok(())
}

#[test]
fn highlights_from_other_sources_and_ambiguous_old_revisions_are_not_repainted()
-> Result<()> {
    let (_directory, mut app, writer, target) =
        reading("Alpha unique passage.")?;
    writer.save_highlight(target.article, "unique", 6, "Feed annotation")?;
    cache(&writer, &target, "Alpha unique passage.")?;
    app.reload()?;
    assert!(app.reader_text.contains("Quote from feed content"));
    let marked = ratatui::style::Color::Rgb(65, 53, 32);
    assert!(
        !app.reader_styled
            .iter()
            .flat_map(|line| &line.spans)
            .any(|span| span.style.bg == Some(marked))
    );
    writer.save_passage(
        target.article,
        &lume::model::Passage {
            quote: "unique".into(),
            anchor: 6,
            source: ContentSource::Full,
            revision: 1,
        },
        "Full annotation",
    )?;
    cache(&writer, &target, "unique appears twice: unique.")?;
    app.reload()?;
    assert!(
        !app.reader_styled
            .iter()
            .take(2)
            .flat_map(|line| &line.spans)
            .any(|span| span.style.bg == Some(marked))
    );
    assert!(
        app.reader_text
            .contains("Quote from Full revision 1; current revision 2")
    );
    Ok(())
}

#[test]
fn failed_refetch_keeps_the_exact_full_reader_cursor_and_visible_page()
-> Result<()> {
    let (_directory, mut app, writer, target) = reading(DISCUSSION)?;
    let full = "A paragraph with enough words for navigation.\n\n".repeat(30);
    cache(&writer, &target, &full)?;
    app.reload()?;
    app.resize_reader(30, 3);
    for _ in 0..8 {
        press(&mut app, KeyCode::Char('j'))?;
    }
    press(&mut app, KeyCode::Char('l'))?;
    let position = (app.scroll, app.reader_cursor, app.reader_column);
    app.article_fetch_started(target.clone());
    writer.document_failure(&target, "HTTP 503")?;

    app.article_fetch_finished(target, Some("HTTP 503"));
    app.apply_document_completion()?;
    app.resize_reader(30, 3);

    assert_eq!((app.scroll, app.reader_cursor, app.reader_column), position);
    assert_eq!(app.content_source, ContentSource::Full);
    assert_eq!(app.body, full);
    assert_eq!(
        app.document.as_ref().context("Cache lost")?.error,
        "HTTP 503"
    );
    Ok(())
}

#[test]
fn stale_full_passage_save_retains_the_draft_and_its_original_quote()
-> Result<()> {
    let (_directory, mut app, writer, target) = reading(DISCUSSION)?;
    cache(&writer, &target, "Alpha beta gamma.")?;
    app.reload()?;
    press(&mut app, KeyCode::Char('v'))?;
    press(&mut app, KeyCode::Char('n'))?;
    app.paste("An interpretation worth keeping.")?;
    cache(&writer, &target, "The publisher replaced this version.")?;

    let saved =
        app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));

    assert!(saved.is_err());
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Draft was lost after a stale save");
    };
    assert_eq!(input.text, "An interpretation worth keeping.");
    let InputKind::Highlight { passage, .. } = &input.kind else {
        anyhow::bail!("Captured passage disappeared");
    };
    assert_eq!(passage.quote, "A");
    assert_eq!(passage.revision, 1);
    assert!(writer.comments(target.article)?.is_empty());
    Ok(())
}

#[test]
fn feed_and_full_keep_independent_reading_positions_when_switching_sources()
-> Result<()> {
    let feed_body = (0..30)
        .map(|i| format!("Feed paragraph {i}.\n\n"))
        .collect::<String>();
    let full_body = (0..30)
        .map(|i| format!("Full paragraph {i}.\n\n"))
        .collect::<String>();
    let (_directory, mut app, writer, target) = reading(&feed_body)?;
    cache(&writer, &target, &full_body)?;
    app.reload()?;
    app.resize_reader(40, 3);
    for _ in 0..8 {
        press(&mut app, KeyCode::Char('j'))?;
    }
    app.save_session()?;
    let full_position = writer
        .cached_document(target.article)?
        .context("Cache lost")?
        .progress;
    assert!(full_position > 0);
    assert_eq!(writer.progress(target.article)?, 0);
    command(&mut app, "content feed")?;
    app.resize_reader(40, 3);
    assert_eq!(app.scroll, 0);
    for _ in 0..6 {
        press(&mut app, KeyCode::Char('j'))?;
    }
    app.save_session()?;
    let feed_position = writer.progress(target.article)?;
    assert!(feed_position > 0);

    command(&mut app, "content full")?;
    app.resize_reader(40, 3);

    assert_eq!(app.line_ranges[app.scroll].start, full_position);
    assert_eq!(writer.progress(target.article)?, feed_position);
    Ok(())
}

#[test]
fn an_old_footer_only_cache_falls_back_without_erasing_either_source()
-> Result<()> {
    let (directory, app, writer, target) = reading("Useful feed summary.")?;
    drop(app);
    cache(&writer, &target, "Previously accepted extraction.")?;
    let connection =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    connection.execute(
        include_str!("../src/store/sql/test_update_document_footer.sql"),
        [target.article],
    )?;
    drop(connection);
    let store = Store::open(&directory.path().join("library.sqlite3"))?;

    let reopened = App::new(store, false)?;

    assert_eq!(reopened.content_source, ContentSource::Feed);
    assert_eq!(reopened.body, "Useful feed summary.");
    assert!(reopened.article_warning.contains("showing feed"));
    assert!(reopened.article_warning.contains("footer"));
    assert_eq!(writer.body(target.article)?, "Useful feed summary.");
    assert!(
        writer
            .cached_document(target.article)?
            .context("Existing cache was deleted")?
            .body
            .contains("Copyright")
    );
    Ok(())
}
