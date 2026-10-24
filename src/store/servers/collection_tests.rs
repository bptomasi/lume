//! Download completions recheck authority without overwriting concurrent work.
use super::*;
use anyhow::Context;

fn page(server: i64, changed: i64) -> RemoteFeedUpdate {
    update(
        server,
        RemoteArticleState {
            unread: true,
            starred: false,
        },
        changed,
    )
}

#[test]
fn a_late_page_and_error_cannot_rewind_another_connection_or_poison_health()
-> Result<()> {
    let mut fixture = mirrored_store()?;
    let mut other =
        Store::open(&fixture._directory.path().join("library.sqlite3"))?;
    assert!(collect(&mut other, &fixture.feed, "Latest", 200, 1000)?);
    let id = other.library()?.articles[0].id;
    other.save_comment(id, None, "Concurrent note")?;
    other.set_starred(id, true)?;
    assert!(!collect(
        &mut fixture.store,
        &fixture.feed,
        "Obsolete",
        100,
        2000
    )?);
    assert!(!fixture.store.record_collection_failure(
        &fixture.feed,
        2000,
        "Obsolete 401",
        3000
    )?);
    let current = remote_scope(&fixture.store)?;
    assert_eq!(current.last_changed, 200);
    let library = fixture.store.library()?;
    assert_eq!(library.articles.len(), 1);
    assert_eq!(library.articles[0].title, "Latest");
    assert!(library.articles[0].starred);
    assert_eq!(fixture.store.comments(id)?[0].body, "Concurrent note");
    assert_eq!(fixture.store.source_failures(fixture.feed.id)?, 0);
    assert!(fixture.store.feeds()?[0].error.is_empty());
    Ok(())
}

#[test]
fn changed_credentials_pause_and_removal_reject_admitted_payloads() -> Result<()>
{
    for action in ["credentials", "pause", "remove"] {
        let mut fixture = mirrored_store()?;
        match action {
            "credentials" => {
                fixture
                    .store
                    .add_server("https://reader.example", "OTHER_TOKEN")?;
            }
            "pause" => {
                fixture.store.set_server_enabled(fixture.server_id, false)?
            }
            _ => fixture.store.remove_server(fixture.server_id)?,
        }
        let changes = fixture.store.connection.total_changes();
        assert!(!collect(
            &mut fixture.store,
            &fixture.feed,
            "Obsolete",
            100,
            2000
        )?);
        assert!(!fixture.store.record_collection_failure(
            &fixture.feed,
            2000,
            "Old error",
            3000
        )?);
        assert_eq!(fixture.store.connection.total_changes(), changes);
        assert!(fixture.store.library()?.articles.is_empty());
    }
    Ok(())
}

#[test]
fn health_publication_failure_rolls_back_content_and_cursor() -> Result<()> {
    let mut fixture = mirrored_store()?;
    fixture.store.connection.execute_batch(include_str!(
        "../sql/test_reject_collected_attempt.sql"
    ))?;
    let result = collect(&mut fixture.store, &fixture.feed, "New", 100, 1000);
    assert!(result.is_err());
    assert!(fixture.store.library()?.articles.is_empty());
    assert!(!remote_scope(&fixture.store)?.initialized);
    fixture.store.connection.execute_batch(include_str!(
        "../sql/test_allow_collected_attempt.sql"
    ))?;
    assert!(collect(
        &mut fixture.store,
        &fixture.feed,
        "New",
        100,
        1000
    )?);
    Ok(())
}

#[test]
fn current_payload_preserves_personal_edits_made_during_download() -> Result<()>
{
    let mut fixture = mirrored_store()?;
    assert!(collect(
        &mut fixture.store,
        &fixture.feed,
        "New",
        100,
        1000
    )?);
    let captured = fixture.store.feeds()?.remove(0);
    let id = fixture.store.library()?.articles[0].id;
    fixture.store.set_queued(id, true)?;
    fixture.store.set_starred(id, true)?;
    fixture
        .store
        .save_comment(id, None, "Personal edit during HTTP")?;
    assert!(collect(
        &mut fixture.store,
        &captured,
        "Correction",
        200,
        2000
    )?);
    let article = fixture.store.article(id)?;
    assert!(article.queued && article.starred);
    assert_eq!(article.title, "Correction");
    assert_eq!(
        fixture.store.comments(id)?[0].body,
        "Personal edit during HTTP"
    );
    assert_eq!(remote_scope(&fixture.store)?.last_changed, 200);
    Ok(())
}

#[test]
fn restarting_history_invalidates_an_inflight_incremental_page() -> Result<()> {
    let mut fixture = mirrored_store()?;
    fixture.store.ingest_remote(
        &fixture.feed,
        "Current",
        &[],
        &page(fixture.server_id, 100),
    )?;
    let captured = fixture
        .store
        .feeds()?
        .into_iter()
        .next()
        .context("Current feed")?;
    fixture.store.restart_server_history(fixture.server_id)?;
    assert!(!collect(
        &mut fixture.store,
        &captured,
        "Obsolete",
        200,
        2000
    )?);
    assert!(!remote_scope(&fixture.store)?.initialized);
    assert!(fixture.store.library()?.articles.is_empty());
    Ok(())
}

fn collect(
    store: &mut Store,
    feed: &crate::model::Feed,
    title: &str,
    changed: i64,
    now: i64,
) -> Result<bool> {
    let remote = feed.server.as_ref().context("Captured remote feed")?;
    store.ingest_collected_remote(
        feed,
        title,
        &[article("miniflux:1", title)],
        &page(remote.server_id, changed),
        now,
    )
}

#[test]
fn changing_root_or_history_settings_discards_an_old_page() -> Result<()> {
    for change_root in [false, true] {
        let mut fixture = mirrored_store()?;
        let path = fixture._directory.path().join("config.toml");
        let session = crate::config::Session::open(&fixture.store, &path)?;
        let mut config = crate::config::decode(&session.initial().document)?;
        if change_root {
            config.servers[0].url = "https://other-reader.test".into();
        } else {
            config.servers[0].sync.pages_per_sync = 1;
        }
        std::fs::write(&path, crate::config::encode(&config)?)?;
        crate::config::Session::open(&fixture.store, &path)?;
        assert!(!collect(
            &mut fixture.store,
            &fixture.feed,
            "Obsolete",
            100,
            1000
        )?);
        let current = fixture.store.feeds()?.remove(0);
        assert!(collect(&mut fixture.store, &current, "Current", 100, 1000)?);
    }
    Ok(())
}

#[test]
fn obsolete_discovery_cannot_archive_feeds_or_publish_health() -> Result<()> {
    for action in ["credentials", "pause", "remove"] {
        let mut fixture = mirrored_store()?;
        let captured = fixture.store.servers()?.remove(0);
        match action {
            "credentials" => {
                fixture.store.add_server(&captured.url, "OTHER_TOKEN")?;
            }
            "pause" => fixture.store.set_server_enabled(captured.id, false)?,
            _ => fixture.store.remove_server(captured.id)?,
        }
        let changes = fixture.store.connection.total_changes();
        assert!(!fixture.store.sync_collected_server_feeds(
            &captured,
            &[],
            1
        )?);
        assert!(!fixture.store.record_collected_server_failure(
            &captured,
            2,
            "Obsolete 401"
        )?);
        assert_eq!(fixture.store.connection.total_changes(), changes);
        if action != "remove" {
            assert!(
                fixture.store.feeds()?[0]
                    .server
                    .as_ref()
                    .context("Mirrored feed")?
                    .present
            );
            assert!(fixture.store.servers()?[0].error.is_empty());
        }
    }
    Ok(())
}

#[test]
fn discovery_and_health_roll_back_together_and_allow_retry() -> Result<()> {
    let mut fixture = mirrored_store()?;
    let captured = fixture.store.servers()?.remove(0);
    fixture.store.connection.execute_batch(include_str!(
        "../sql/test_reject_discovery_attempt.sql"
    ))?;
    assert!(
        fixture
            .store
            .sync_collected_server_feeds(&captured, &[], 10)
            .is_err()
    );
    assert!(
        fixture.store.feeds()?[0]
            .server
            .as_ref()
            .context("Feed after rollback")?
            .present
    );
    fixture.store.connection.execute_batch(include_str!(
        "../sql/test_allow_discovery_attempt.sql"
    ))?;
    fixture
        .store
        .record_server_attempt(captured.id, 11, "Previous failure")?;
    assert!(
        fixture
            .store
            .sync_collected_server_feeds(&captured, &[], 12)?
    );
    assert!(
        !fixture.store.feeds()?[0]
            .server
            .as_ref()
            .context("Archived feed")?
            .present
    );
    assert!(fixture.store.servers()?[0].error.is_empty());
    Ok(())
}
