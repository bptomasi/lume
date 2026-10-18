//! Catalog label upgrades retain saved-search identities and private state.

use anyhow::{Result, anyhow};
use lume::{
    app::App,
    catalog,
    model::{SmartFilter, SmartList, SmartListRename, View, Workspace},
    store::Store,
};
use serde::Deserialize;

#[derive(Deserialize)]
struct HistoricalSearch {
    name: String,
    previous_name: Option<String>,
}

fn legacy_lists(store: &Store) -> Result<Vec<(SmartList, String)>> {
    let searches: Vec<HistoricalSearch> =
        serde_json::from_str(include_str!("../assets/reading/searches.json"))?;
    let mut names = searches
        .into_iter()
        .filter_map(|search| search.previous_name.map(|old| (old, search.name)))
        .collect::<Vec<_>>();
    names.push((
        "19 · Segurança computacional".into(),
        catalog::SECURITY_LIST.into(),
    ));
    let mut expected = Vec::new();
    for (old, new) in names {
        store.save_smart_list(
            &old,
            &SmartFilter {
                text: format!("Personal criteria for {new}"),
                starred: true,
                days: Some(45),
                ..SmartFilter::default()
            },
        )?;
        expected.push((named(store, &old)?, new));
    }
    Ok(expected)
}

fn named(store: &Store, name: &str) -> Result<SmartList> {
    store
        .smart_lists()?
        .into_iter()
        .find(|list| list.name == name)
        .ok_or_else(|| anyhow!("Missing saved search {name}"))
}

#[test]
fn historical_labels_keep_criteria_identities_and_the_active_view() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("upgrade.db"))?;
    let expected = legacy_lists(&store)?;
    let selected = View::Smart(expected[0].0.id);
    store.save_workspace(&Workspace {
        view: selected.clone(),
        ..Workspace::default()
    })?;

    catalog::install(&store)?;
    let repeated = catalog::install(&store)?;

    assert_eq!(store.smart_lists()?.len(), 46);
    assert_eq!(repeated.added_searches, 0);
    for (old, name) in expected {
        let upgraded = named(&store, &name)?;
        assert_eq!(upgraded.id, old.id);
        assert_eq!(upgraded.filter, old.filter);
        assert!(store.smart_lists()?.iter().all(|s| s.name != old.name));
    }
    let app = App::new(store, false)?;
    assert_eq!(app.view, selected);
    Ok(())
}

#[test]
fn colliding_labels_keep_both_queries_with_a_stable_english_suffix()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("collision.db"))?;
    let personal = SmartFilter {
        text: "private criteria".into(),
        ..SmartFilter::default()
    };
    for name in [catalog::SECURITY_LIST, "19 · Security (imported)"] {
        store.save_smart_list(name, &SmartFilter::default())?;
    }
    store.save_smart_list("19 · Segurança computacional", &personal)?;
    let old = named(&store, "19 · Segurança computacional")?;

    catalog::install(&store)?;
    let repeated = catalog::install(&store)?;

    let imported = named(&store, "19 · Security (imported 2)")?;
    assert_eq!(imported.id, old.id);
    assert_eq!(imported.filter, personal);
    assert_eq!(named(&store, catalog::SECURITY_LIST)?.filter.text, "");
    assert_eq!(store.smart_lists()?.len(), 48);
    assert_eq!(repeated.added_searches, 0);
    Ok(())
}

#[test]
fn custom_names_and_ai_criteria_are_not_reclassified() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("custom.db"))?;
    let personal = SmartFilter {
        text: "quantization".into(),
        unread: true,
        ..SmartFilter::default()
    };
    for name in [catalog::AI_LIST, "My research", "Security notes"] {
        store.save_smart_list(name, &personal)?;
    }
    let before = store.smart_lists()?;

    catalog::install(&store)?;

    for old in before {
        let current = named(&store, &old.name)?;
        assert_eq!(current.id, old.id);
        assert_eq!(current.filter, old.filter);
    }
    Ok(())
}

#[test]
fn conditional_rename_rolls_back_the_batch_on_stale_or_conflicting_names()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("atomic.db"))?;
    for name in ["First", "Second", "Occupied"] {
        store.save_smart_list(name, &SmartFilter::default())?;
    }
    let first = named(&store, "First")?;
    let second = named(&store, "Second")?;
    for (expected_name, name) in [
        ("Stale", "New second"),
        ("Second", "Occupied"),
        ("Second", ""),
    ] {
        let result = store.rename_smart_lists(&[
            SmartListRename {
                id: first.id,
                expected_name: first.name.clone(),
                name: "New first".into(),
            },
            SmartListRename {
                id: second.id,
                expected_name: expected_name.into(),
                name: name.into(),
            },
        ]);
        assert!(result.is_err(), "Accepted invalid rename {expected_name}");
        assert_eq!(named(&store, "First")?.id, first.id);
        assert_eq!(named(&store, "Second")?.id, second.id);
        assert_eq!(store.smart_lists()?.len(), 3);
    }
    Ok(())
}

#[test]
fn archive_label_upgrade_preserves_publisher_content_and_personal_state()
-> Result<()> {
    let (_directory, store, article) = archive_library()?;
    let body = store.body(article)?;
    let comment = store.comments(article)?.remove(0);

    catalog::install(&store)?;

    let feed = store.article(article)?.feed_id;
    let current = store.feeds()?.into_iter().find(|item| item.id == feed);
    assert_eq!(
        current.map(|item| item.title),
        Some("The Morning Paper — Archive".into())
    );
    assert_eq!(store.body(article)?, body);
    assert_eq!(store.comments(article)?[0].id, comment.id);
    assert_eq!(store.comments(article)?[0].body, comment.body);
    assert!(store.article(article)?.starred);
    assert!(!store.article(article)?.unread);
    assert!(store.rename_feed_title_if_current(
        "https://blog.acolyer.org/feed/",
        "The Morning Paper — Archive",
        "Publisher title",
    )?);
    catalog::install(&store)?;
    assert!(!store.rename_feed_title_if_current(
        "https://blog.acolyer.org/feed/",
        "The Morning Paper — Archive",
        "Replacement",
    )?);
    Ok(())
}

fn archive_library() -> Result<(tempfile::TempDir, Store, i64)> {
    let directory = tempfile::tempdir()?;
    let mut store = Store::open(&directory.path().join("archive.db"))?;
    store.subscribe("https://blog.acolyer.org/feed/")?;
    let feed = store.feeds()?.remove(0);
    store.ingest(
        &feed,
        "The Morning Paper - acervo",
        &[lume::model::IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: "original-fixture".into(),
            title: "Original publisher fixture".into(),
            url: String::new(),
            author: "Fixture author".into(),
            published: 2_000_000_000,
            body: "Publisher content stays byte-for-byte intact.".into(),
        }],
    )?;
    let article = store.library()?.articles[0].id;
    store.set_starred(article, true)?;
    store.set_unread(article, false)?;
    store.save_comment(article, None, "Personal research")?;
    Ok((directory, store, article))
}
