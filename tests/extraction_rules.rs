//! Extraction rules require review and retain the captured source authority.

mod common;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lume::{
    app::{App, Mode},
    model::{ArticleTarget, FeedOrigin, RulePreview},
    source::{self, article},
    store::Store,
};

const PAGE: &str = r#"
<html><body>
<nav><a href="/home">Home</a><a href="/about">About</a></nav>
<main id="technical"><h1>Commit boundaries</h1>
<p>Durable writes require an explicit transaction boundary.</p>
<pre><code class="language-sql">SELECT id FROM journal;
</code></pre></main>
<aside>Accept all cookies</aside>
</body></html>
"#;

#[test]
fn a_rule_preview_waits_for_a_draft_and_cancel_changes_no_source_or_cache()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let preview = proposed(&store, feed.id)?;
    let reader = Store::open(&directory.path().join("library.sqlite3"))?;
    let mut app = App::new(store, false)?;
    app.key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE))?;
    app.rule_preview_finished(preview.clone());
    assert!(!app.present_rule_preview());
    assert!(matches!(app.mode, Mode::Input(_)));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert!(app.present_rule_preview());
    assert!(matches!(app.mode, Mode::Document(_)));
    app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE))?;
    assert_eq!(reader.article_selector(feed.id)?, "");
    assert!(reader.cached_document(preview.target.article)?.is_none());

    app.rule_preview_finished(preview.clone());
    assert!(app.present_rule_preview());
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    assert_eq!(reader.article_selector(feed.id)?, "#technical");
    assert_eq!(
        reader
            .cached_document(preview.target.article)?
            .context("Missing reviewed cache")?
            .body,
        preview.document.body
    );
    Ok(())
}

fn proposed(store: &Store, feed: i64) -> Result<RulePreview> {
    let article = store.library()?.articles.remove(0);
    Ok(RulePreview {
        target: ArticleTarget {
            article: article.id,
            url: article.url,
        },
        feed,
        previous: store.article_selector(feed)?,
        selector: "#technical".into(),
        document: article::extract_with_selector(
            PAGE,
            "https://journal.example/a",
            "#technical",
        )?,
    })
}

#[test]
fn css_container_keeps_code_and_rejects_invalid_missing_and_ambiguous_rules()
-> Result<()> {
    let document = article::extract_with_selector(
        PAGE,
        "https://journal.example/a",
        "#technical",
    )?;

    assert!(document.body.contains("Durable writes require"));
    assert!(document.body.contains("~~~sql\nSELECT id FROM journal;"));
    assert!(!document.body.contains("Accept all cookies"));
    assert!(!document.body.contains("About"));
    for selector in ["[", "#missing", "a", "aside", "nav"] {
        assert!(
            article::extract_with_selector(
                PAGE,
                "https://journal.example/a",
                selector
            )
            .is_err(),
            "{selector}"
        );
    }
    assert!(article::rules::validate_selector(&"x".repeat(1025)).is_err());
    assert!(
        article::rules::warnings("A useful short note.")
            .iter()
            .any(|warning| warning.contains("Short"))
    );
    Ok(())
}

#[test]
fn preview_changes_nothing_until_confirmation_and_survives_reopen() -> Result<()>
{
    let (directory, store, feed) = common::seeded_store()?;
    let preview = proposed(&store, feed.id)?;
    let original = store.body(preview.target.article)?;
    assert_eq!(store.article_selector(feed.id)?, "");
    assert!(store.cached_document(preview.target.article)?.is_none());

    store.confirm_article_rule(&preview)?;

    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.article_selector(feed.id)?, "#technical");
    assert_eq!(reopened.body(preview.target.article)?, original);
    assert_eq!(
        reopened
            .cached_document(preview.target.article)?
            .context("Missing cache")?
            .body,
        preview.document.body
    );
    Ok(())
}

#[test]
fn stale_rule_or_deleted_article_cannot_overwrite_another_confirmed_preview()
-> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    let stale = proposed(&store, feed.id)?;
    store.confirm_article_rule(&stale)?;
    let mut changed = stale.clone();
    changed.selector = "main".into();

    assert!(store.confirm_article_rule(&changed).is_err());

    assert_eq!(store.article_selector(feed.id)?, "#technical");
    changed.previous = "#technical".into();
    store.set_deleted(changed.target.article, true)?;
    assert!(store.confirm_article_rule(&changed).is_err());
    assert_eq!(store.article_selector(feed.id)?, "#technical");
    Ok(())
}

#[test]
fn summary_provenance_and_collection_health_are_durable() -> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let parsed = source::parse(
        include_bytes!("fixtures/partial-content.xml"),
        &feed.url,
    )?;
    store.ingest(&feed, "Partial feed", &parsed.articles)?;
    let summary = store.library()?.articles.into_iter().find(|article| {
        store.feed_origin(article.id).ok() == Some(FeedOrigin::Summary)
    });
    assert!(summary.is_some(), "Summary fallback lost parser provenance");
    store.record_source_attempt(feed.id, 100, "Publisher returned 429", 400)?;

    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    let health = reopened.source_health()?.remove(0);
    assert_eq!(
        (
            health.attempted_at,
            health.succeeded_at,
            health.retry_at,
            health.failures
        ),
        (100, 0, 400, 1)
    );
    assert_eq!(health.error, "Publisher returned 429");
    reopened.record_source_attempt(feed.id, 500, "", 0)?;
    let health = reopened.source_health()?.remove(0);
    assert_eq!(
        (
            health.attempted_at,
            health.succeeded_at,
            health.retry_at,
            health.failures
        ),
        (500, 500, 0, 0)
    );
    Ok(())
}

#[test]
fn source_health_distinguishes_unattempted_sources_and_shows_their_urls()
-> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    let mut app = App::new(store, false)?;
    app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE))?;
    app.paste("source-health")?;
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;

    let Mode::Picker(picker) = &app.mode else {
        anyhow::bail!("Source health did not open");
    };
    assert!(picker.choices[0].title.ends_with(" · NEW"));
    assert!(picker.preview.contains(&feed.url));
    assert!(picker.preview.contains("Last attempt:"));
    Ok(())
}
