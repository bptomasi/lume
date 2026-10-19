//! Local recommendation behavior with independent expected article identities.

use anyhow::{Context, Result};
use lume::{
    model::{
        ArticleQuery, FeedOrigin, IncomingArticle, Interest,
        RecommendationFeedback as Feedback, RecommendationProfile, TagTarget,
        View,
    },
    store::Store,
};

fn profile() -> RecommendationProfile {
    RecommendationProfile {
        interests: vec![Interest {
            name: "Kernel networking".into(),
            weight: 9,
            query: "conntrack OR \"connection tracking\" OR cilium".into(),
        }],
        ..RecommendationProfile {
            interests: Vec::new(),
            ..Default::default()
        }
    }
}

fn article(id: &str, title: &str, body: &str) -> IncomingArticle {
    IncomingArticle {
        external_id: id.into(),
        title: title.into(),
        url: format!("https://engineering.example/{id}"),
        author: "Ada".into(),
        published: 100,
        body: body.into(),
        body_origin: FeedOrigin::Feed,
    }
}

fn fixture() -> Result<(tempfile::TempDir, Store)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("library.sqlite3"))?;
    let id = store.subscribe("https://engineering.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|f| f.id == id)
        .context("Missing feed")?;
    store.ingest(
        &feed,
        "Engineering",
        &[
            article(
                "kernel",
                "Understanding connection tracking",
                "conntrack state machines\n\n```c\nint state = 0;\n```",
            ),
            article(
                "announcement",
                "Introducing Cilium webinar",
                "cilium release announcement",
            ),
            article("unrelated", "Sourdough recipes", "Fermentation and flour"),
        ],
    )?;
    store.save_recommendation_profile(&profile())?;
    settle(&store)?;
    Ok((directory, store))
}

fn settle(store: &Store) -> Result<()> {
    for _ in 0..100 {
        if !store.refresh_recommendations()?.remaining {
            return Ok(());
        }
    }
    anyhow::bail!("Classification did not finish within 6400 articles")
}

fn page(store: &Store, text: &str) -> Result<Vec<i64>> {
    Ok(store
        .query_articles(&ArticleQuery {
            view: View::Recommended,
            text: text.into(),
            offset: 0,
            limit: 100,
            retained: None,
        })?
        .articles
        .into_iter()
        .map(|article| article.id)
        .collect())
}

fn id(store: &Store, key: &str) -> Result<i64> {
    store
        .library()?
        .articles
        .into_iter()
        .find(|article| article.url.ends_with(key))
        .map(|article| article.id)
        .context("Fixture article missing")
}

#[test]
fn related_vocabulary_and_observed_evidence_order_articles_without_a_model()
-> Result<()> {
    let (_directory, store) = fixture()?;
    let kernel = id(&store, "kernel")?;
    let announcement = id(&store, "announcement")?;
    assert_eq!(page(&store, "")?, vec![kernel, announcement]);
    let evidence = store.recommendation_evidence(kernel)?;
    assert_eq!(evidence.interests, vec!["Kernel networking"]);
    assert_eq!(evidence.headline, vec!["Kernel networking"]);
    assert_eq!(evidence.evaluated_from, "Feed content");
    assert!(evidence.code);
    assert!(store.recommendation_evidence(announcement)?.announcement);
    assert_eq!(page(&store, "title:webinar")?, vec![announcement]);
    Ok(())
}

#[test]
fn private_notes_and_reading_progress_never_become_publisher_evidence()
-> Result<()> {
    let (_directory, store) = fixture()?;
    let unrelated = id(&store, "unrelated")?;
    store.save_comment(unrelated, None, "conntrack and connection tracking")?;
    store.save_progress(unrelated, 10)?;
    assert!(!store.recommendations_dirty()?);
    assert!(!page(&store, "")?.contains(&unrelated));
    let mut changed = profile();
    changed.interests[0].query = "notes:conntrack".into();
    assert!(store.save_recommendation_profile(&changed).is_err());
    changed.interests[0].query = "conntrack".into();
    store.save_recommendation_profile(&changed)?;
    settle(&store)?;
    assert!(!page(&store, "")?.contains(&unrelated));
    Ok(())
}

#[test]
fn unchanged_refresh_and_idempotent_ingestion_leave_no_text_work() -> Result<()>
{
    let (_directory, mut store) = fixture()?;
    assert_eq!(store.refresh_recommendations()?.processed, 0);
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Engineering",
        &[article(
            "kernel",
            "Understanding connection tracking",
            "conntrack state machines\n\n```c\nint state = 0;\n```",
        )],
    )?;
    assert!(!store.recommendations_dirty()?);
    store.ingest(
        &feed,
        "Engineering",
        &[article("unrelated", "Cilium implementation", "conntrack")],
    )?;
    assert_eq!(store.refresh_recommendations()?.processed, 1);
    assert!(page(&store, "")?.contains(&id(&store, "unrelated")?));
    Ok(())
}

#[test]
fn explicit_feedback_is_reversible_and_read_state_is_not_approval() -> Result<()>
{
    let (_directory, store) = fixture()?;
    let kernel = id(&store, "kernel")?;
    store.set_unread(kernel, false)?;
    settle(&store)?;
    assert!(!page(&store, "")?.contains(&kernel));
    assert_eq!(store.recommendation_feedback(kernel)?, None);
    store.set_unread(kernel, true)?;
    store.set_recommendation_feedback(kernel, Some(Feedback::Useful))?;
    settle(&store)?;
    assert_eq!(
        store.recommendation_feedback(kernel)?,
        Some(Feedback::Useful)
    );
    for feedback in [
        Feedback::NotInterested,
        Feedback::TooBasic,
        Feedback::TooAdvanced,
    ] {
        store.set_recommendation_feedback(kernel, Some(feedback))?;
        settle(&store)?;
        assert!(!page(&store, "")?.contains(&kernel));
    }
    store.set_recommendation_feedback(kernel, None)?;
    settle(&store)?;
    assert!(page(&store, "")?.contains(&kernel));
    Ok(())
}

#[test]
fn exclusions_priorities_source_preferences_and_empty_profiles_are_durable()
-> Result<()> {
    let (directory, store) = fixture()?;
    let mut preferences = profile();
    preferences.exclude = "title:webinar".into();
    preferences.sources.insert(store.feeds()?[0].id, -3);
    preferences.include_read = true;
    store.save_recommendation_profile(&preferences)?;
    assert!(page(&store, "")?.is_empty(), "Old profile must not leak");
    settle(&store)?;
    assert_eq!(page(&store, "")?, vec![id(&store, "kernel")?]);
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;
    assert_eq!(reopened.recommendation_profile()?, preferences);
    preferences.interests.clear();
    store.save_recommendation_profile(&preferences)?;
    settle(&store)?;
    assert!(page(&store, "")?.is_empty());
    Ok(())
}

#[test]
fn invalid_profiles_preserve_existing_preferences_and_parameterize_literals()
-> Result<()> {
    let (_directory, store) = fixture()?;
    for query in [
        "is:unread",
        "notes:private",
        "after:2026-01-01",
        "badfield:x",
    ] {
        let mut invalid = profile();
        invalid.interests[0].query = query.into();
        assert!(
            store.save_recommendation_profile(&invalid).is_err(),
            "{query}"
        );
    }
    let mut invalid = profile();
    invalid.interests.push(invalid.interests[0].clone());
    assert!(store.save_recommendation_profile(&invalid).is_err());
    invalid = profile();
    invalid.minutes = Some(0);
    assert!(store.save_recommendation_profile(&invalid).is_err());
    assert_eq!(store.recommendation_profile()?, profile());
    let mut literal = profile();
    literal.interests[0].query = "\"'; DROP TABLE articles; --\"".into();
    store.save_recommendation_profile(&literal)?;
    settle(&store)?;
    assert_eq!(store.library()?.articles.len(), 3);
    assert!(page(&store, "")?.is_empty());
    Ok(())
}

#[test]
fn trash_and_undo_preserve_personal_feedback_but_hide_deleted_articles()
-> Result<()> {
    let (_directory, store) = fixture()?;
    let kernel = id(&store, "kernel")?;
    store.set_recommendation_feedback(kernel, Some(Feedback::Useful))?;
    store.set_deleted(kernel, true)?;
    assert!(!page(&store, "")?.contains(&kernel));
    assert!(store.set_recommendation_feedback(kernel, None).is_err());
    store.undo()?;
    settle(&store)?;
    assert!(page(&store, "")?.contains(&kernel));
    assert_eq!(
        store.recommendation_feedback(kernel)?,
        Some(Feedback::Useful)
    );
    Ok(())
}

#[test]
fn explicit_article_tags_invalidate_their_owned_classification() -> Result<()> {
    let (_directory, store) = fixture()?;
    let unrelated = id(&store, "unrelated")?;
    let mut preferences = profile();
    preferences.interests[0].query = "tag:study".into();
    store.save_recommendation_profile(&preferences)?;
    settle(&store)?;
    assert!(page(&store, "")?.is_empty());
    store.set_tags(TagTarget::Article(unrelated), &["study".into()])?;
    settle(&store)?;
    assert_eq!(page(&store, "")?, vec![unrelated]);
    store.undo()?;
    settle(&store)?;
    assert!(page(&store, "")?.is_empty());
    Ok(())
}

#[test]
fn full_article_cache_and_changed_url_invalidate_exactly_the_owned_evidence()
-> Result<()> {
    use lume::model::{ArticleTarget, ExtractedDocument};
    let (_directory, mut store) = fixture()?;
    let unrelated = id(&store, "unrelated")?;
    let target = ArticleTarget {
        article: unrelated,
        url: store.article(unrelated)?.url,
    };
    store.cache_document(
        &target,
        &ExtractedDocument {
            final_url: target.url.clone(),
            body: "Detailed conntrack implementation".into(),
        },
    )?;
    settle(&store)?;
    assert!(page(&store, "")?.contains(&unrelated));
    assert_eq!(
        store.recommendation_evidence(unrelated)?.evaluated_from,
        "Feed + cached full article"
    );
    store.document_failure(&target, "Temporary network failure")?;
    assert!(!store.recommendations_dirty()?);
    let feed = store.feeds()?.remove(0);
    let mut changed =
        article("unrelated", "Sourdough recipes", "Flour and fermentation");
    changed.url = "https://engineering.example/replaced".into();
    store.ingest(&feed, "Engineering", &[changed])?;
    assert!(store.recommendation_evidence(unrelated).is_err());
    settle(&store)?;
    assert!(!page(&store, "")?.contains(&unrelated));
    Ok(())
}

#[test]
fn canonical_duplicates_share_a_recommendation_and_negative_feedback()
-> Result<()> {
    let (_directory, mut store) = fixture()?;
    let kernel = id(&store, "kernel")?;
    let feed_id = store.subscribe("https://community.example/rss")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == feed_id)
        .context("Feed missing")?;
    let duplicate =
        article("kernel", "Understanding connection tracking", "conntrack");
    store.ingest(&feed, "Community", &[duplicate])?;
    settle(&store)?;
    assert_eq!(page(&store, "")?.len(), 2);
    store.set_recommendation_feedback(kernel, Some(Feedback::NotInterested))?;
    settle(&store)?;
    assert_eq!(page(&store, "")?, vec![id(&store, "announcement")?]);
    store.set_recommendation_feedback(kernel, None)?;
    settle(&store)?;
    assert_eq!(page(&store, "")?.len(), 2);
    Ok(())
}

#[test]
fn batches_are_bounded_and_resume_after_reopen_without_loading_the_library()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("batch.sqlite3");
    let mut store = Store::open(&path)?;
    let feed_id = store.subscribe("https://engineering.example/feed")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == feed_id)
        .context("Feed missing")?;
    let articles = (0..130)
        .map(|n| article(&n.to_string(), "conntrack", "state machines"))
        .collect::<Vec<_>>();
    store.ingest(&feed, "Engineering", &articles)?;
    store.save_recommendation_profile(&profile())?;
    let first = store.refresh_recommendations()?;
    assert_eq!(first.processed, 64);
    assert!(first.remaining);
    assert_eq!(first.total, 64);
    drop(store);
    let store = Store::open(&path)?;
    let second = store.refresh_recommendations()?;
    assert_eq!(second.processed, 64);
    assert!(second.remaining);
    let last = store.refresh_recommendations()?;
    assert_eq!(last.processed, 2);
    assert!(!last.remaining);
    assert_eq!(last.total, 100);
    assert_eq!(store.refresh_recommendations()?.processed, 0);
    Ok(())
}

#[test]
fn phrase_and_boolean_queries_keep_their_meaning_and_do_not_expand_to_notes()
-> Result<()> {
    let (_directory, store) = fixture()?;
    let mut preferences = profile();
    preferences.interests[0].query =
        "\"connection tracking\" AND NOT title:webinar".into();
    store.save_recommendation_profile(&preferences)?;
    settle(&store)?;
    assert_eq!(page(&store, "")?, vec![id(&store, "kernel")?]);
    assert!(
        store
            .recommendation_evidence(id(&store, "kernel")?)?
            .headline
            .is_empty(),
        "A negated query must not claim positive title evidence"
    );
    Ok(())
}

#[test]
fn rank_order_controls_pagination_position_and_retained_read_selection()
-> Result<()> {
    let (_directory, store) = fixture()?;
    let kernel = id(&store, "kernel")?;
    let announcement = id(&store, "announcement")?;
    let mut query = ArticleQuery {
        view: View::Recommended,
        text: String::new(),
        offset: 0,
        limit: 1,
        retained: None,
    };
    assert_eq!(store.query_articles(&query)?.articles[0].id, kernel);
    let (offset, found) = store
        .page_containing(&query, announcement)?
        .context("Article lost")?;
    assert_eq!(offset, 1);
    assert_eq!(found.articles[0].id, announcement);
    store.set_unread(kernel, false)?;
    query.retained = Some(kernel);
    assert_eq!(store.query_articles(&query)?.articles[0].id, kernel);
    query.retained = None;
    assert_eq!(store.query_articles(&query)?.articles[0].id, announcement);
    Ok(())
}

#[test]
fn diversity_balances_sources_and_topics_with_stable_duplicate_selection() {
    use lume::recommendation::{Candidate, select};
    let candidate = |id, feed, score, topic: &str, canonical: &str| Candidate {
        article: id,
        feed,
        score,
        topic: topic.into(),
        canonical: canonical.into(),
        published: 100,
    };
    let rows = vec![
        candidate(1, 1, 100, "kernel", "one"),
        candidate(2, 1, 99, "kernel", "two"),
        candidate(3, 2, 90, "data", "three"),
        candidate(4, 3, 80, "identity", "one"),
    ];
    assert_eq!(select(rows.clone()), vec![1, 3, 2]);
    assert_eq!(select(rows), vec![1, 3, 2]);
}

#[test]
fn time_budget_filters_by_cached_reading_duration_and_can_be_removed()
-> Result<()> {
    let (_directory, mut store) = fixture()?;
    let kernel = id(&store, "kernel")?;
    let feed = store.feeds()?.remove(0);
    let body = format!("conntrack {}", "mechanism ".repeat(500));
    store.ingest(
        &feed,
        "Engineering",
        &[article(
            "kernel",
            "Understanding connection tracking",
            &body,
        )],
    )?;
    let mut preferences = profile();
    preferences.minutes = Some(1);
    store.save_recommendation_profile(&preferences)?;
    settle(&store)?;
    assert_eq!(page(&store, "")?, vec![id(&store, "announcement")?]);
    preferences.minutes = None;
    store.save_recommendation_profile(&preferences)?;
    settle(&store)?;
    assert!(page(&store, "")?.contains(&kernel));
    Ok(())
}

#[test]
fn explicit_source_preference_changes_the_order_without_fabricating_matches()
-> Result<()> {
    let (_directory, mut store) = fixture()?;
    let feed_id = store.subscribe("https://another.example/rss")?;
    let feed = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.id == feed_id)
        .context("Feed missing")?;
    store.ingest(
        &feed,
        "Another journal",
        &[
            article("other", "conntrack", "conntrack state machines"),
            article("bread", "Bread", "Flour"),
        ],
    )?;
    let mut preferences = profile();
    preferences.sources.insert(feed_id, 10);
    store.save_recommendation_profile(&preferences)?;
    settle(&store)?;
    let found = page(&store, "")?;
    assert_eq!(found[0], id(&store, "other")?);
    assert!(!found.contains(&id(&store, "bread")?));
    Ok(())
}
