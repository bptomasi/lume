//! Batched counts must match independent destination queries across mutations.
mod common;
use anyhow::{Context, Result};
use lume::{
    model::{SmartFilter, View},
    store::Store,
};

fn compare(store: &Store, views: &[View]) -> Result<()> {
    let expected = views
        .iter()
        .map(|view| store.view_count(view))
        .collect::<Result<Vec<_>>>()?;
    assert_eq!(store.view_counts(views)?, expected);
    Ok(())
}

#[test]
fn batched_counts_preserve_scopes_duplicates_empty_sources_and_personal_data()
-> Result<()> {
    let (_directory, store, feed) = common::seeded_store()?;
    let id = store.library()?.articles[0].id;
    let folder = store.save_collection("Personal", Some(id))?;
    let views = destinations(&store, feed.id, folder)?;
    compare(&store, &views)?;
    store.set_starred(id, true)?;
    store.set_queued(id, true)?;
    store.save_progress(id, 12)?;
    store.save_comment(id, None, "A saved note")?;
    store.set_unread(id, false)?;
    compare(&store, &views)?;
    store.set_deleted(id, true)?;
    compare(&store, &views)?;
    store.set_deleted(id, false)?;
    store.unsubscribe(feed.id)?;
    compare(&store, &views)?;
    assert_eq!(store.comments(id)?.len(), 1);
    assert!(store.view_counts(&[View::Smart(-1)]).is_err());
    assert!(store.view_counts(&[])?.is_empty());
    Ok(())
}

#[test]
fn canonical_representatives_and_remote_archives_match_individual_counts()
-> Result<()> {
    let (_directory, mut store, feed) = common::seeded_store()?;
    let server = store.add_server("https://reader.test", "TOKEN")?;
    store.sync_server_feeds(
        server,
        &[lume::model::RemoteFeedCandidate {
            remote_id: 7,
            title: "Remote".into(),
            remote_url: feed.url.clone(),
            local_url: "https://reader.test/v1/feeds/7".into(),
        }],
    )?;
    let remote = store
        .feeds()?
        .into_iter()
        .find(|feed| feed.server.is_some())
        .context("Remote feed")?;
    let parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    store.ingest(&remote, &remote.title, &parsed.articles)?;
    let views = [
        View::All,
        View::Grouped,
        View::Feed(remote.id),
        View::Server(server),
    ];
    compare(&store, &views)?;
    assert_eq!(store.view_counts(&views)?[1], 1);
    store.sync_server_feeds(server, &[])?;
    compare(&store, &views)?;
    assert_eq!(store.view_count(&View::Server(server))?, 1);
    Ok(())
}

fn destinations(store: &Store, feed: i64, folder: i64) -> Result<Vec<View>> {
    let mut views = vec![
        View::All,
        View::Inbox,
        View::Read,
        View::Trash,
        View::Starred,
        View::Queue,
        View::Annotated,
        View::Continue,
        View::Grouped,
        View::Recommended,
        View::SinceLastVisit(1),
        View::Collection(folder),
        View::Feed(feed),
        View::Server(-1),
        View::Feed(-1),
        View::Collection(-1),
        View::All,
    ];
    views.extend(saved_views(store)?);
    Ok(views)
}

fn saved_views(store: &Store) -> Result<Vec<View>> {
    let mut views = Vec::new();
    for filter in [
        SmartFilter::default(),
        SmartFilter {
            unread: true,
            ..Default::default()
        },
        SmartFilter {
            text: "systems OR is:read".into(),
            ..Default::default()
        },
    ] {
        for copy in 0..2 {
            let name = format!("List {} {copy}", views.len());
            store.save_smart_list(&name, &filter)?;
            let list = store
                .smart_lists()?
                .into_iter()
                .find(|list| list.name == name)
                .context("Saved list")?;
            views.push(View::Smart(list.id));
        }
    }
    Ok(views)
}
