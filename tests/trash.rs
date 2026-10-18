//! Trash is a durable visibility state, independent of publisher refreshes.

mod common;

use std::collections::BTreeSet;

use anyhow::Result;
use lume::{
    model::{ArticleQuery, IncomingArticle, SmartFilter, View},
    source,
    store::Store,
};

fn query(view: View, text: &str) -> ArticleQuery {
    ArticleQuery {
        view,
        text: text.into(),
        offset: 0,
        limit: 10,
        retained: None,
    }
}

#[test]
fn deletion_hides_every_normal_destination_but_preserves_personal_state()
-> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    let body = store.body(article)?;
    store.set_unread(article, false)?;
    store.set_starred(article, true)?;
    store.set_queued(article, true)?;
    store.save_comment(article, None, "searchableannotation")?;
    let folder = store.create_collection("Reference")?;
    store.add_member(folder, article)?;
    store.save_smart_list("Everything", &SmartFilter::default())?;
    let smart = store.smart_lists()?.remove(0).id;

    assert!(store.set_deleted(article, true)?);

    assert_only_trash_contains_article(
        &store, article, feed.id, folder, smart,
    )?;
    assert_eq!(store.view_count(&View::Trash)?, 1);
    assert!(store.library()?.articles.is_empty());
    assert_eq!(store.body(article)?, body);
    assert_eq!(store.comments(article)?[0].body, "searchableannotation");
    assert_eq!(
        store.article_memberships(article)?,
        BTreeSet::from([folder])
    );
    let metadata = store.article(article)?;
    assert!(metadata.starred && metadata.queued && !metadata.unread);
    assert_eq!(
        store
            .query_articles(&query(View::All, "searchableannotation"))?
            .total,
        0
    );
    assert_eq!(
        store
            .query_articles(&query(View::Trash, "searchableannotation"))?
            .total,
        1
    );
    Ok(())
}

fn assert_only_trash_contains_article(
    store: &Store,
    article: i64,
    feed: i64,
    folder: i64,
    smart: i64,
) -> Result<()> {
    for view in [
        View::All,
        View::Inbox,
        View::Read,
        View::Starred,
        View::Queue,
        View::Annotated,
        View::Feed(feed),
        View::Collection(folder),
        View::Smart(smart),
    ] {
        let mut request = query(view.clone(), "");
        request.retained = Some(article);
        assert_eq!(store.query_articles(&request)?.total, 0, "{view:?}");
        assert_eq!(store.view_count(&view)?, 0);
    }
    Ok(())
}

#[test]
fn deleted_articles_stay_deleted_across_refresh_reopen_and_undo() -> Result<()>
{
    let (directory, mut store, feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.save_comment(article, None, "Keep my note")?;
    store.clear_history()?;
    store.set_deleted(article, true)?;
    let mut refresh =
        source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    refresh.articles[0].title = "Publisher correction".into();
    refresh.articles[0].body = "Updated publisher body".into();

    store.ingest(&feed, &refresh.title, &refresh.articles)?;
    drop(store);
    let reopened = Store::open(&directory.path().join("library.sqlite3"))?;

    assert!(reopened.is_deleted(article)?);
    assert_eq!(reopened.view_count(&View::All)?, 0);
    assert_eq!(reopened.view_count(&View::Trash)?, 1);
    assert_eq!(reopened.body(article)?, "Updated publisher body");
    assert!(reopened.undo()?);
    assert!(!reopened.is_deleted(article)?);
    assert_eq!(reopened.article(article)?.title, "Publisher correction");
    assert_eq!(reopened.comments(article)?[0].body, "Keep my note");
    assert!(!reopened.undo()?);
    Ok(())
}

#[test]
fn idempotent_and_missing_target_requests_do_not_consume_undo_entries()
-> Result<()> {
    let (_directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.clear_history()?;
    assert!(!store.set_deleted(article, false)?);
    assert!(store.set_deleted(-1, true).is_err());
    assert!(store.set_deleted(-1, false).is_err());
    assert!(!store.undo()?);
    assert!(store.set_deleted(article, true)?);
    assert!(!store.set_deleted(article, true)?);
    assert!(store.set_deleted(article, false)?);
    assert!(!store.set_deleted(article, false)?);
    assert!(store.undo()?);
    assert!(store.is_deleted(article)?);
    assert!(store.undo()?);
    assert!(!store.is_deleted(article)?);
    assert!(!store.undo()?);
    Ok(())
}

#[test]
fn deletion_preserves_global_position_and_trash_pagination_with_ties()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let rows = (0..4)
        .map(|index| IncomingArticle {
            body_origin: lume::model::FeedOrigin::Feed,
            external_id: format!("tie-{index}"),
            title: format!("Tied article {index}"),
            url: String::new(),
            author: String::new(),
            published: 2_000_000_000,
            body: "Tie-breaking fixture".into(),
        })
        .collect::<Vec<_>>();
    store.ingest(&feed, &feed.title, &rows)?;
    let ordered = store.query_articles(&query(View::All, "tie"))?.articles;
    store.set_deleted(ordered[0].id, true)?;
    store.set_deleted(ordered[2].id, true)?;

    assert_eq!(store.article_position(ordered[1].id)?, 0);
    assert_eq!(store.article_position(ordered[3].id)?, 1);
    assert!(store.article_position(ordered[0].id).is_err());
    let mut request = query(View::Trash, "tie");
    request.offset = 1;
    request.limit = 1;
    let page = store.query_articles(&request)?;
    assert_eq!(page.total, 2);
    assert_eq!(page.articles.len(), 1);
    assert_eq!(page.articles[0].title, "Tied article 1");
    Ok(())
}

#[test]
fn version_two_upgrade_keeps_legacy_data_workspace_and_undo_compatible()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("version-two.db");
    version_two_fixture(&path)?;

    let store = Store::open(&path)?;

    assert!(!store.is_deleted(42)?);
    assert_eq!(store.view_count(&View::Trash)?, 0);
    assert_eq!(store.comments(42)?[0].body, "Migration keeps my insight");
    assert_eq!(store.progress(42)?, 7);
    assert_eq!(store.article_memberships(42)?, BTreeSet::from([3]));
    assert_eq!(store.workspace()?.article, Some(42));
    assert!(store.article(42)?.queued);
    assert!(store.undo()?);
    assert!(store.article(42)?.unread);
    assert!(!store.article(42)?.starred);
    assert!(!store.article(42)?.queued);
    drop(store);
    assert_eq!(Store::open(&path)?.view_count(&View::All)?, 1);
    Ok(())
}

fn version_two_fixture(path: &std::path::Path) -> Result<()> {
    use refinery::{Migration, Runner};
    let mut connection = rusqlite::Connection::open(path)?;
    connection
        .execute_batch(include_str!("../src/store/sql/create_schema.sql"))?;
    connection.execute_batch(include_str!(
        "../src/store/sql/test_setup_legacy_workspace.sql"
    ))?;
    let migrations = [
        Migration::unapplied(
            "V1__index_owned_library",
            include_str!(
                "../src/store/sql/migrations/V1__index_owned_library.sql"
            ),
        )?,
        Migration::unapplied(
            "V2__extend_reading_workspace",
            include_str!(
                "../src/store/sql/migrations/V2__extend_reading_workspace.sql"
            ),
        )?,
    ];
    Runner::new(&migrations)
        .set_grouped(true)
        .run(&mut connection)?;
    connection.execute_batch(include_str!(
        "../src/store/sql/test_setup_trash_migration.sql"
    ))?;
    Ok(())
}

#[test]
fn notebook_exports_omit_trash_while_explicit_article_export_remains_available()
-> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.save_comment(article, None, "Private retained note")?;
    store.set_deleted(article, true)?;
    let notebook = directory.path().join("notebook.md");
    let single = directory.path().join("article.md");

    lume::exchange::markdown::notebook(&store, &notebook)?;
    lume::exchange::markdown::article(&store, article, &single)?;

    assert!(
        !std::fs::read_to_string(notebook)?.contains("Private retained note")
    );
    assert!(std::fs::read_to_string(single)?.contains("Private retained note"));
    Ok(())
}

#[test]
fn failed_history_write_rolls_back_article_visibility() -> Result<()> {
    let (directory, store, _feed) = common::seeded_store()?;
    let article = store.library()?.articles[0].id;
    store.clear_history()?;
    let database =
        rusqlite::Connection::open(directory.path().join("library.sqlite3"))?;
    database.execute_batch(include_str!(
        "../src/store/sql/test_create_failing_history_trigger.sql"
    ))?;

    assert!(store.set_deleted(article, true).is_err());

    assert!(!store.is_deleted(article)?);
    assert_eq!(store.view_count(&View::All)?, 1);
    assert_eq!(store.view_count(&View::Trash)?, 0);
    assert!(!store.undo()?);
    Ok(())
}
