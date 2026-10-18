//! Captured batches, occurrence grouping and reversible visual preferences.

mod common;

use std::collections::BTreeSet;

use anyhow::{Context, Result};
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{Action, App, Focus, InputBatch, Mode},
    model::{ArticleQuery, BulkEdit, TagTarget, View},
    source,
    store::Store,
    ui,
};
use ratatui::{Terminal, backend::TestBackend, style::Color};

fn key(app: &mut App, code: KeyCode) -> Result<Action> {
    app.key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn command(app: &mut App, text: &str) -> Result<Action> {
    key(app, KeyCode::Char(':'))?;
    app.paste(text)?;
    key(app, KeyCode::Enter)
}

#[test]
fn captured_bulk_changes_are_one_atomic_undo_and_stale_targets_roll_back()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let ids = store
        .library()?
        .articles
        .iter()
        .map(|article| article.id)
        .collect::<BTreeSet<_>>();
    store.clear_history()?;
    store.edit_articles(&ids, &BulkEdit::Favorite(true))?;
    assert_eq!(store.view_count(&View::Starred)?, ids.len());
    assert!(store.undo()?);
    assert_eq!(store.view_count(&View::Starred)?, 0);
    assert!(!store.undo()?);

    let mut invalid = ids;
    invalid.insert(i64::MAX);
    assert!(
        store
            .edit_articles(&invalid, &BulkEdit::Favorite(true))
            .is_err()
    );
    assert_eq!(store.view_count(&View::Starred)?, 0);
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn article_tags_and_highlight_tags_have_independent_undo_and_folder_ownership()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let folder = store.create_collection("Research")?;
    store.add_member(folder, article)?;
    store.save_highlight(article, "Keep", 0, "Independent note")?;
    let comment = store.comments(article)?[0].id;
    store.set_tags(TagTarget::Article(article), &["platform".into()])?;
    store.set_tags(TagTarget::Comment(comment), &["consensus".into()])?;
    assert_eq!(store.tags(TagTarget::Article(article))?, ["platform"]);
    assert_eq!(store.tags(TagTarget::Comment(comment))?, ["consensus"]);
    assert!(store.undo()?);
    assert!(store.tags(TagTarget::Comment(comment))?.is_empty());
    assert_eq!(store.tags(TagTarget::Article(article))?, ["platform"]);
    assert!(store.article_memberships(article)?.contains(&folder));
    Ok(())
}

#[test]
fn exact_url_occurrences_group_without_merging_notes_or_original_links()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let original = store.library()?.articles[0].clone();
    add_community_occurrence(&mut store, &original, &feed)?;
    let occurrences = store.article_occurrences(original.id)?;
    assert_eq!(occurrences.len(), 2);
    assert!(
        occurrences
            .iter()
            .any(|article| article.url == original.url)
    );
    assert!(
        occurrences
            .iter()
            .any(|article| article.url.contains("utm_source"))
    );
    let all = store.view_count(&View::All)?;
    assert_eq!(store.view_count(&View::Grouped)?, all - 1);
    let other = occurrences
        .iter()
        .find(|article| article.id != original.id)
        .context("Occurrence missing")?;
    store.save_comment(original.id, None, "Private original note")?;
    assert!(store.comments(other.id)?.is_empty());
    assert_eq!(
        store.body(other.id)?,
        "[Discussion](https://community.example/thread)"
    );
    Ok(())
}

#[test]
fn sidebar_pins_collapse_and_expansion_persist_without_deleting_sources()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.set_view(View::Starred)?;
    command(&mut app, "pin")?;
    assert_eq!(app.navigation[0].view, View::Starred);
    command(&mut app, "collapse feeds")?;
    assert!(
        !app.navigation
            .iter()
            .any(|item| matches!(item.view, View::Feed(_)))
    );
    assert!(app.navigation.iter().any(
        |item| matches!(&item.view, View::Group(group) if group == "FEEDS")
    ));
    app.focus = Focus::Library;
    key(&mut app, KeyCode::Enter)?;
    assert!(
        app.navigation
            .iter()
            .any(|item| matches!(item.view, View::Feed(_)))
    );
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.workspace()?.pins, [View::Starred]);
    assert_eq!(reopened.feeds()?.len(), 1);
    Ok(())
}

#[test]
fn visual_preview_cancels_or_commits_with_actual_themed_cells() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "settings")?;
    key(&mut app, KeyCode::Right)?;
    assert_eq!(app.workspace.theme, "nord");
    let mut terminal = Terminal::new(TestBackend::new(120, 32))?;
    terminal.draw(|frame| ui::draw(frame, &app))?;
    assert_eq!(
        terminal.backend().buffer()[(0, 0)].bg,
        Color::Rgb(52, 60, 75)
    );
    key(&mut app, KeyCode::Esc)?;
    assert_eq!(app.workspace.theme, "lume");
    command(&mut app, "settings")?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Down)?;
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Enter)?;
    let saved =
        Store::open(&directory.path().join("library.sqlite3"))?.workspace()?;
    assert_eq!((saved.theme.as_str(), saved.reading_width), ("nord", 83));
    Ok(())
}

#[test]
fn custom_motion_key_is_not_coalesced_and_draft_editing_keeps_its_own_keys()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    command(&mut app, "bind j toc")?;
    let mut batch = InputBatch::new(vec![Event::Key(KeyEvent::new(
        KeyCode::Char('j'),
        KeyModifiers::NONE,
    ))]);
    batch.dispatch_next(&mut app)?;
    assert!(matches!(app.mode, Mode::Picker(_)));
    key(&mut app, KeyCode::Esc)?;
    key(&mut app, KeyCode::Char(':'))?;
    key(&mut app, KeyCode::Char('j'))?;
    let Mode::Input(input) = &app.mode else {
        anyhow::bail!("Binding intercepted editor input");
    };
    assert_eq!(input.text, "j");
    key(&mut app, KeyCode::Esc)?;
    assert!(command(&mut app, "bind not-a-key toc").is_err());
    assert!(!app.workspace.keybindings.contains_key("not-a-key"));
    Ok(())
}

#[test]
fn continuing_and_since_visit_use_durable_progress_and_collection_time()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.save_progress(article, 5)?;
    assert_eq!(store.view_count(&View::Continue)?, 1);
    let future = chrono::Utc::now().timestamp() + 10;
    assert_eq!(store.view_count(&View::SinceLastVisit(future))?, 0);
    let page = store.query_articles(&ArticleQuery {
        view: View::SinceLastVisit(1),
        text: String::new(),
        offset: 0,
        limit: 50,
        retained: None,
    })?;
    assert_eq!(page.total, store.view_count(&View::All)?);
    assert!(View::SinceLastVisit(1).same_destination(&View::SinceLastVisit(2)));
    Ok(())
}

#[test]
fn hiding_a_focused_panel_in_settings_returns_focus_to_the_reader() -> Result<()>
{
    let (_directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.focus = Focus::Library;
    command(&mut app, "settings")?;
    for _ in 0..5 {
        key(&mut app, KeyCode::Down)?;
    }
    key(&mut app, KeyCode::Right)?;
    key(&mut app, KeyCode::Enter)?;
    assert!(!app.workspace.show_library);
    assert_eq!(app.focus, Focus::Reader);
    Ok(())
}

fn add_community_occurrence(
    store: &mut Store,
    original: &lume::model::Article,
    feed: &lume::model::Feed,
) -> Result<()> {
    store.subscribe("https://community.example/feed")?;
    let community = store
        .feeds()?
        .into_iter()
        .find(|item| item.id != feed.id)
        .context("Community missing")?;
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &community.url)?;
    let mut url = url::Url::parse(&original.url)?;
    url.query_pairs_mut().append_pair("utm_source", "community");
    parsed.articles[0].url = url.to_string();
    parsed.articles[0].body =
        "[Discussion](https://community.example/thread)".into();
    store.ingest(&community, "Community", &parsed.articles[..1])?;
    Ok(())
}

#[test]
fn occurrence_grouping_applies_search_before_selecting_a_representative()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let original = store.library()?.articles[0].clone();
    add_community_occurrence(&mut store, &original, &feed)?;
    let filtered = store.query_articles(&ArticleQuery {
        view: View::Grouped,
        text: format!("source:\"{}\"", original.feed),
        offset: 0,
        limit: 50,
        retained: None,
    })?;
    assert!(
        filtered
            .articles
            .iter()
            .any(|article| article.id == original.id)
    );
    Ok(())
}

#[test]
fn exiting_during_visual_preview_discards_unconfirmed_settings() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    let before = app.workspace.clone();
    command(&mut app, "settings")?;
    key(&mut app, KeyCode::Right)?;
    assert_ne!(app.workspace.theme, before.theme);

    let action =
        app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL))?;
    assert_eq!(action, Action::Quit);
    app.end_session()?;

    let saved =
        Store::open(&directory.path().join("library.sqlite3"))?.workspace()?;
    assert_eq!(saved.theme, before.theme);
    assert!(saved.last_visit > 0);
    Ok(())
}

#[test]
fn grouped_view_chooses_latest_publication_before_insertion_identity()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let original = store.library()?.articles.remove(0);
    let mut parsed =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].external_id = "late-imported-old-occurrence".into();
    parsed.articles[0].title = "An older repost".into();
    parsed.articles[0].published = original.published - 86_400;
    store.ingest(&feed, "Journal", &parsed.articles)?;

    let page = store.query_articles(&ArticleQuery {
        view: View::Grouped,
        text: String::new(),
        offset: 0,
        limit: 50,
        retained: None,
    })?;
    assert_eq!(page.total, 1);
    assert_eq!(page.articles[0].id, original.id);
    assert_eq!(page.articles[0].published, original.published);
    Ok(())
}
