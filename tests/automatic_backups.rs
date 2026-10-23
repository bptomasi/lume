//! Automatic bundles preserve WAL state and rotate only this library's owned files.
mod common;
use anyhow::{Context, Result};
use lume::{
    backups,
    config::{self, backups::Backups},
    media::cache,
    model::OfflineStatus,
    store::Store,
};
use std::{
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

fn configure(store: &Store, root: &Path, policy: Backups) -> Result<()> {
    let path = root.join("config.toml");
    let session = config::Session::open(store, &path)?;
    let mut document = config::decode(&session.initial().document)?;
    document.backups = policy;
    std::fs::write(&path, config::encode(&document)?)?;
    config::Session::open(store, &path)?;
    Ok(())
}
fn run(store: &Store, now: i64) -> Result<PathBuf> {
    backups::run(store, now, &AtomicBool::new(false))?
        .context("Backup was cancelled")
}

#[test]
fn automatic_is_opt_in_and_successful_schedule_survives_restart() -> Result<()>
{
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    let cancel = AtomicBool::new(false);
    assert!(backups::automatic(&store, 1_800_000_000, &cancel)?.is_none());
    assert!(!root.join("backups").exists());
    configure(
        &store,
        root,
        Backups {
            automatic: true,
            interval_hours: 1,
            ..Default::default()
        },
    )?;
    assert!(
        backups::automatic(&store, 1_800_000_000, &AtomicBool::new(true))?
            .is_none()
    );
    let path = backups::automatic(&store, 1_800_000_001, &cancel)?
        .context("Due backup")?;
    backups::verify(&path)?;
    let reopened = Store::open(&root.join("library.sqlite3"))?;
    assert!(backups::automatic(&reopened, 1_800_000_002, &cancel)?.is_none());
    assert!(backups::next_check(&reopened, 1_800_000_002)?.as_secs() > 3500);
    assert!(backups::automatic(&reopened, 1_800_003_602, &cancel)?.is_some());
    Ok(())
}

#[test]
fn rotation_preserves_other_libraries_and_untracked_files() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    let shared = root.join("shared-backups");
    configure(
        &store,
        root,
        Backups {
            keep: 2,
            directory: shared.clone(),
            ..Default::default()
        },
    )?;
    let first = run(&store, 10)?;
    let protected = run(&store, 11)?;
    std::fs::write(protected.join("personal.txt"), b"Untracked evidence")?;
    let unrelated = shared.join("lume-backup-unrelated");
    std::fs::create_dir(&unrelated)?;
    std::fs::write(unrelated.join("private"), b"Keep")?;
    let other_root = root.join("other-library");
    let other = Store::open(&other_root.join("library.sqlite3"))?;
    configure(
        &other,
        &other_root,
        Backups {
            directory: shared.clone(),
            ..Default::default()
        },
    )?;
    let other_backup = run(&other, 11)?;
    let newest = run(&store, 12)?;
    let last = run(&store, 13)?;
    assert!(!first.exists());
    assert!(protected.join("personal.txt").is_file());
    assert!(other_backup.join("manifest.json").is_file());
    assert!(unrelated.join("private").is_file());
    backups::verify(&newest)?;
    backups::verify(&last)?;
    Ok(())
}

#[test]
fn offline_bundle_restores_snapshot_notes_configuration_and_pinned_originals()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    configure(
        &store,
        root,
        Backups {
            include_offline: true,
            ..Default::default()
        },
    )?;
    let article = store.library()?.articles[0].id;
    store.save_comment(article, None, "Before backup")?;
    let address = "https://images.test/original.svg";
    cache::write(&root.join("images/offline"), address, b"Pinned original")?;
    prepared_checkpoint(&store, article)?;
    let snapshot = run(&store, 100)?;
    store.save_comment(article, None, "After backup")?;
    let restored = root.join("restored");
    backups::restore(&snapshot, &restored)?;
    let recovered = Store::open(&restored.join("library.sqlite3"))?;
    assert_eq!(recovered.comments(article)?.len(), 1);
    assert_eq!(recovered.comments(article)?[0].body, "Before backup");
    assert!(
        config::read_document(&restored.join("config.toml"))?
            .backups
            .include_offline
    );
    assert_eq!(
        std::fs::read(cache::path(&restored.join("images/offline"), address))?,
        b"Pinned original"
    );
    let checkpoint =
        recovered.offline_state()?.context("Offline checkpoint")?;
    assert!(!checkpoint.running);
    assert_eq!(checkpoint.items[0].images, 1);
    assert_eq!(checkpoint.items[0].status, OfflineStatus::Ready);
    assert!(backups::restore(&snapshot, &restored).is_err());
    assert_eq!(store.comments(article)?.len(), 2);
    Ok(())
}

#[test]
fn corrupted_bundle_fails_before_publishing_any_restore_destination()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let snapshot = run(&store, 100)?;
    let database = snapshot.join("library.sqlite3");
    let mut bytes = std::fs::read(&database)?;
    bytes[100] ^= 1;
    std::fs::write(&database, bytes)?;
    let restored = directory.path().join("rejected");
    assert!(backups::verify(&snapshot).is_err());
    assert!(backups::restore(&snapshot, &restored).is_err());
    assert!(!restored.exists());
    Ok(())
}

#[test]
fn explicit_bundles_are_no_clobber_and_can_omit_offline_files() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    let output = root.join("explicit");
    cache::write(
        &root.join("images/offline"),
        "https://images.test/pin",
        b"Pinned",
    )?;
    cache::write(
        &root.join("images"),
        "https://images.test/disposable",
        b"Disposable",
    )?;
    backups::snapshot_existing(&root.join("library.sqlite3"), &output, false)?;
    assert!(!output.join("images").exists());
    assert!(
        backups::snapshot_existing(
            &root.join("library.sqlite3"),
            &output,
            true
        )
        .is_err()
    );
    let restored = root.join("restored");
    backups::restore(&output, &restored)?;
    assert_eq!(
        Store::open(&restored.join("library.sqlite3"))?
            .library()?
            .articles[0]
            .id,
        store.library()?.articles[0].id
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn symlinks_and_path_traversal_are_rejected_without_reading_external_files()
-> Result<()> {
    use std::os::unix::fs::symlink;
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    configure(
        &store,
        root,
        Backups {
            include_offline: true,
            ..Default::default()
        },
    )?;
    let offline = root.join("images/offline");
    std::fs::create_dir_all(&offline)?;
    let outside = root.join("external");
    std::fs::write(&outside, b"Private")?;
    symlink(&outside, cache::path(&offline, "https://images.test/link"))?;
    assert!(backups::run(&store, 1, &AtomicBool::new(false)).is_err());
    assert_eq!(std::fs::read(&outside)?, b"Private");
    let snapshot = backups::snapshot_existing(
        &root.join("library.sqlite3"),
        &root.join("bundle"),
        false,
    );
    snapshot?;
    let manifest = root.join("bundle/manifest.json");
    let mut value: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&manifest)?)?;
    value["files"][0]["name"] = "../external".into();
    std::fs::write(&manifest, serde_json::to_vec(&value)?)?;
    assert!(
        backups::restore(&root.join("bundle"), &root.join("rejected")).is_err()
    );
    assert!(!root.join("rejected").exists());
    assert_eq!(std::fs::read(&outside)?, b"Private");
    Ok(())
}

fn prepared_checkpoint(store: &Store, article: i64) -> Result<()> {
    store.set_queued(article, true)?;
    let mut state = lume::offline::capture(
        store,
        lume::model::View::Queue,
        Default::default(),
    )?;
    state.running = true;
    state.items[0].status = OfflineStatus::Ready;
    state.items[0].images = 1;
    state.items[0].bytes = store.body(article)?.len() as u64 + 100;
    store.save_offline_state(&state)
}

#[test]
fn restored_snapshots_without_assets_keep_targets_and_report_missing_images()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let root = directory.path();
    let article = store.library()?.articles[0].id;
    prepared_checkpoint(&store, article)?;
    let plain = root.join("plain.sqlite3");
    store.backup_to(&plain)?;
    let bundle = root.join("bundle");
    backups::snapshot_existing(&root.join("library.sqlite3"), &bundle, false)?;
    for (snapshot, name) in
        [(&plain, "plain-restored"), (&bundle, "bundle-restored")]
    {
        let output = root.join(name);
        if snapshot.is_dir() {
            backups::restore(snapshot, &output)?;
        } else {
            Store::restore_snapshot(snapshot, &output)?;
        }
        let restored = Store::open(&output.join("library.sqlite3"))?;
        let state = restored.offline_state()?.context("Restored checkpoint")?;
        assert!(!state.running);
        assert_eq!(state.items[0].article, article);
        assert_eq!(state.items[0].images, 0);
        assert_eq!(state.items[0].status, OfflineStatus::Partial);
        assert_eq!(state.items[0].bytes, store.body(article)?.len() as u64);
        assert!(state.items[0].error.contains("resume"));
        assert!(restored.article(article)?.queued);
    }
    let original = store.offline_state()?.context("Original checkpoint")?;
    assert!(original.running);
    assert_eq!(original.items[0].images, 1);
    Ok(())
}

#[test]
fn retargeted_offline_items_do_not_claim_availability_after_restore()
-> Result<()> {
    let (directory, mut store, feed) = common::seeded_store()?;
    let root = directory.path();
    let article = store.library()?.articles[0].id;
    prepared_checkpoint(&store, article)?;
    let mut parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    parsed.articles[0].url = "https://publisher.test/changed-target".into();
    store.ingest(&feed, &feed.title, &parsed.articles)?;
    for include_offline in [false, true] {
        let bundle = root.join(format!("bundle-{include_offline}"));
        backups::snapshot_existing(
            &root.join("library.sqlite3"),
            &bundle,
            include_offline,
        )?;
        let output = root.join(format!("restored-{include_offline}"));
        backups::restore(&bundle, &output)?;
        let restored = Store::open(&output.join("library.sqlite3"))?;
        let state = restored.offline_state()?.context("Checkpoint")?;
        assert_eq!(state.items[0].article, article);
        assert_eq!(state.items[0].status, OfflineStatus::Failed);
        assert_eq!(state.items[0].images, 0);
        assert_eq!(state.items[0].bytes, 0);
        assert!(state.items[0].error.contains("unavailable"));
        assert_eq!(restored.article(article)?.url, parsed.articles[0].url);
    }
    Ok(())
}
