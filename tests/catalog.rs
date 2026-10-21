//! The curated selection installs through real persistence contracts.

use std::collections::BTreeSet;

use anyhow::Result;
use lume::{catalog, model::SmartFilter, store::Store};

#[test]
fn reading_selection_contains_distinct_sources_across_the_requested_topics()
-> Result<()> {
    let feeds = catalog::feeds()?;
    let urls = feeds
        .iter()
        .map(|feed| feed.url.as_str())
        .collect::<BTreeSet<_>>();

    assert_eq!(feeds.len(), 98);
    assert_eq!(urls.len(), feeds.len());
    for expected in [
        "https://kubernetes.io/feed.xml",
        "https://lobste.rs/rss",
        "https://www.keycloak.org/rss.xml",
        "https://openfga.dev/blog/rss.xml",
        "https://planet.postgresql.org/rss20.xml",
        "https://debezium.io/blog.atom",
        "https://developer.confluent.io/blog/rss.xml",
        "https://opentelemetry.io/blog/index.xml",
        "https://projectzero.google/feed.xml",
        "https://portswigger.net/research/rss",
        "https://blog.trailofbits.com/index.xml",
        "https://huggingface.co/blog/feed.xml",
        "https://research.google/blog/rss/",
        "https://rss.arxiv.org/rss/cs.CL+cs.LG+cs.AI",
    ] {
        assert!(
            urls.contains(expected),
            "Missing requested topic: {expected}"
        );
    }
    Ok(())
}

fn group_library(
    name: &str,
) -> Result<(tempfile::TempDir, Store, lume::model::SmartList)> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("security.db"))?;
    catalog::install(&store)?;
    let list = store
        .smart_lists()?
        .into_iter()
        .find(|list| list.name == name)
        .ok_or_else(|| anyhow::anyhow!("Missing source group {name}"))?;
    Ok((directory, store, list))
}

#[test]
fn security_group_includes_future_source_articles_without_keyword_matches()
-> Result<()> {
    use lume::model::{ArticleQuery, View};
    let (_directory, mut store, list) = group_library(catalog::SECURITY_LIST)?;
    assert_eq!(list.filter.feeds.len(), 8);
    let query = ArticleQuery {
        view: View::Smart(list.id),
        text: String::new(),
        offset: 0,
        limit: 10,
        retained: None,
    };
    assert_eq!(store.query_articles(&query)?.total, 0);
    publish(&mut store, "https://kubernetes.io/feed.xml", "outside")?;
    assert_eq!(store.query_articles(&query)?.total, 0);
    publish(&mut store, "https://projectzero.google/feed.xml", "future")?;
    assert_eq!(store.query_articles(&query)?.total, 1);
    Ok(())
}

fn publish(store: &mut Store, url: &str, external_id: &str) -> Result<()> {
    let source = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.url == url)
        .ok_or_else(|| anyhow::anyhow!("Missing source {url}"))?;
    store.ingest(
        &source,
        "Research",
        &[lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: external_id.into(),
            title: "A new mechanism".into(),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000,
            body: "No matching keyword is required.".into(),
        }],
    )?;
    Ok(())
}

#[test]
fn editing_a_source_group_preserves_its_membership() -> Result<()> {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use lume::{app::App, model::View};
    let (_directory, store, list) = group_library(catalog::SECURITY_LIST)?;
    let mut app = App::new(store, false)?;
    app.set_view(View::Smart(list.id))?;
    for character in ":smart".chars() {
        app.key(KeyEvent::new(KeyCode::Char(character), KeyModifiers::NONE))?;
    }
    app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))?;
    app.key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL))?;
    assert_eq!(
        app.library
            .smart_lists
            .iter()
            .find(|s| s.id == list.id)
            .map(|s| &s.filter.feeds),
        Some(&list.filter.feeds)
    );
    Ok(())
}

#[test]
fn legacy_filters_remain_unrestricted_and_invalid_groups_are_not_saved()
-> Result<()> {
    let legacy: SmartFilter =
        serde_json::from_str(r#"{"text":"vulnerability"}"#)?;
    assert!(legacy.feeds.is_empty());
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("invalid.db"))?;
    for feeds in [vec![-1], vec![0], vec![1; 129]] {
        assert!(
            store
                .save_smart_list(
                    "Invalid",
                    &SmartFilter {
                        feeds,
                        ..SmartFilter::default()
                    }
                )
                .is_err()
        );
    }
    assert!(store.smart_lists()?.is_empty());
    Ok(())
}

#[test]
fn installation_is_idempotent_and_preserves_existing_searches_and_subscriptions()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("reading.sqlite3"))?;
    store.subscribe("https://personal.example/feed")?;
    let personal_filter = SmartFilter {
        text: "scheduler memory".into(),
        ..SmartFilter::default()
    };
    for name in ["01 · Kubernetes", catalog::SECURITY_LIST] {
        store.save_smart_list(name, &personal_filter)?;
    }

    let first = catalog::install(&store)?;
    let second = catalog::install(&store)?;

    assert_eq!(first.added_feeds, 98);
    assert_eq!(first.added_searches, 44);
    assert_eq!(second.added_feeds, 0);
    assert_eq!(second.added_searches, 0);
    assert_eq!(store.feeds()?.len(), 99);
    let searches = store.smart_lists()?;
    assert_eq!(searches.len(), 46);
    for name in ["01 · Kubernetes", catalog::SECURITY_LIST] {
        let existing = searches.iter().find(|search| search.name == name);
        assert_eq!(
            existing.map(|search| &search.filter),
            Some(&personal_filter)
        );
    }
    Ok(())
}

#[test]
fn starter_cli_installs_into_the_requested_database_and_rejects_demo()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("reading.sqlite3");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(&path)
        .arg("starter")
        .output()?;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(Store::open(&path)?.feeds()?.len(), 98);

    let rejected = std::process::Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env("LUME_DATA_DIR", directory.path())
        .args(["--demo", "starter"])
        .output()?;

    assert!(!rejected.status.success());
    assert!(String::from_utf8_lossy(&rejected.stderr).contains("fictional"));
    Ok(())
}

#[test]
fn ai_group_includes_every_selected_source_and_excludes_other_topics()
-> Result<()> {
    use lume::model::{ArticleQuery, View};
    let (_directory, mut store, list) = group_library(catalog::AI_LIST)?;
    let sources = catalog::ai_feeds()?;
    assert_eq!(sources.len(), 14);
    assert_eq!(list.filter.feeds.len(), sources.len());
    for source in sources {
        publish(&mut store, &source.url, "new-research")?;
    }
    publish(&mut store, "https://kubernetes.io/feed.xml", "outside")?;
    let query = ArticleQuery {
        view: View::Smart(list.id),
        text: String::new(),
        offset: 0,
        limit: 30,
        retained: None,
    };

    let page = store.query_articles(&query)?;

    assert_eq!(page.total, 14);
    assert!(
        page.articles
            .iter()
            .all(|a| list.filter.feeds.contains(&a.feed_id))
    );
    Ok(())
}

#[test]
fn ai_topic_searches_match_real_text_across_sources() -> Result<()> {
    use lume::model::{ArticleQuery, View};
    let (_directory, mut store, _group) = group_library(catalog::AI_LIST)?;
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "Engineering fixture",
        &[lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "model-research".into(),
            title: "Training transformer LLMs".into(),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000,
            body: "Fine-tuning, retrieval, inference, evaluation, agents, \
                   multimodal models and GPU infrastructure."
                .into(),
        }],
    )?;
    let topics = store.smart_lists()?.into_iter().filter(|list| {
        list.name
            .get(..2)
            .is_some_and(|prefix| ("21"..="30").contains(&prefix))
    });
    let mut matched = 0;
    for topic in topics {
        let page = store.query_articles(&ArticleQuery {
            view: View::Smart(topic.id),
            text: String::new(),
            offset: 0,
            limit: 10,
            retained: None,
        })?;
        assert_eq!(page.total, 1, "Topic did not match: {}", topic.name);
        matched += 1;
    }
    assert_eq!(matched, 10);
    Ok(())
}
