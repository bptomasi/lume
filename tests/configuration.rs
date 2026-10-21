//! Portable configuration, durable pending edits and preservation boundaries.

mod common;

use std::fs;

use anyhow::Result;
use lume::{
    config::{self, Session},
    model::SmartFilter,
    store::Store,
};

#[test]
fn new_library_starts_neutral_with_a_minimal_configuration() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    session.flush()?;
    let text = fs::read_to_string(&path)?;
    assert_eq!(text.lines().count(), 2);
    let config = config::decode(&text)?;
    assert!(config.feeds.is_empty());
    assert!(config.folders.is_empty());
    assert!(config.smart_lists.is_empty());
    assert!(config.recommendations.interests.is_empty());
    assert!(store.recommendation_profile()?.interests.is_empty());
    Ok(())
}

#[test]
fn user_changes_publish_without_reformatting_untouched_preferences()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let original = include_str!("fixtures/configuration/commented.toml");
    fs::write(&path, original)?;
    let session = Session::open(&store, &path)?;
    assert_eq!(fs::read_to_string(&path)?, original);
    store.subscribe("https://journal.example/feed")?;
    store.create_collection("Weekend")?;
    store.save_smart_list(
        "Science",
        &SmartFilter {
            text: "astronomy OR geology".into(),
            ..Default::default()
        },
    )?;
    session.flush()?;
    let published = fs::read_to_string(&path)?;
    assert!(published.contains("# My reader"));
    assert!(published.contains("theme = 'nord' # chosen palette"));
    let config = config::decode(&published)?;
    assert_eq!(config.feeds[0].url, "https://journal.example/feed");
    assert_eq!(config.folders[0].name, "Weekend");
    assert_eq!(config.smart_lists[0].query, "astronomy OR geology");
    Ok(())
}

#[test]
fn an_existing_library_exports_definitions_without_private_reading_data()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    let folder = store.create_collection("Keep this folder")?;
    store.add_member(folder, article.id)?;
    store.set_starred(article.id, true)?;
    store.save_comment(article.id, None, "PRIVATE NOTE SENTINEL")?;
    let path = directory.path().join("config.toml");
    Session::open(&store, &path)?;
    let text = fs::read_to_string(&path)?;
    assert!(!text.contains("PRIVATE NOTE SENTINEL"));
    assert!(!text.contains(&article.title));
    assert!(store.article(article.id)?.starred);
    assert!(store.article_memberships(article.id)?.contains(&folder));
    assert_eq!(store.comments(article.id)?[0].body, "PRIVATE NOTE SENTINEL");
    assert_eq!(config::decode(&text)?.folders[0].name, "Keep this folder");
    Ok(())
}

#[test]
fn independent_file_and_library_edits_merge_without_losing_either() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    store.create_collection("From the interface")?;
    let mut external = config::decode(&fs::read_to_string(&path)?)?;
    external.ui.theme = "gruvbox".into();
    fs::write(&path, config::encode(&external)?)?;
    session.flush()?;
    let merged = config::decode(&fs::read_to_string(&path)?)?;
    assert_eq!(merged.ui.theme, "gruvbox");
    assert_eq!(merged.folders[0].name, "From the interface");
    assert_eq!(store.workspace()?.theme, "gruvbox");
    Ok(())
}

#[test]
fn conflicting_edits_preserve_the_file_and_pending_library_change() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut workspace = store.workspace()?;
    workspace.theme = "gruvbox".into();
    store.save_workspace(&workspace)?;
    let external = "schema_version = 1\n[ui]\ntheme = 'nord'\n";
    fs::write(&path, external)?;
    let error = session
        .flush()
        .err()
        .ok_or_else(|| anyhow::anyhow!("Conflict was silently accepted"))?;
    assert!(error.to_string().contains("conflict"));
    assert_eq!(fs::read_to_string(&path)?, external);
    assert_eq!(store.workspace()?.theme, "gruvbox");
    Ok(())
}

#[test]
fn malformed_files_never_partially_replace_the_library() -> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let path = directory.path().join("config.toml");
    Session::open(&store, &path)?;
    for invalid in [
        "schema_version = 999\n",
        "schema_version = 1\nunknown = true\n",
        "[[feeds]]\nid = 'broken'\nurl = 'file:///etc/passwd'\n",
        "[[smart_lists]]\nid = 'missing'\nname = 'Missing'\nfeed = 'unknown'\n",
    ] {
        fs::write(&path, invalid)?;
        assert!(Session::open(&store, &path).is_err());
        assert_eq!(store.feeds()?[0].id, feed.id);
        assert_eq!(fs::read_to_string(&path)?, invalid);
    }
    Ok(())
}

#[test]
fn removing_and_restoring_a_folder_preserves_its_memberships() -> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    let folder = store.create_collection("Study")?;
    store.add_member(folder, article.id)?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let original = config::decode(&fs::read_to_string(&path)?)?;
    let mut removed = original.clone();
    removed.folders.clear();
    fs::write(&path, config::encode(&removed)?)?;
    session.flush()?;
    assert!(store.catalog()?.collections.is_empty());
    assert!(store.article_memberships(article.id)?.contains(&folder));
    fs::write(&path, config::encode(&original)?)?;
    session.flush()?;
    assert_eq!(store.catalog()?.collections[0].id, folder);
    assert!(store.article_memberships(article.id)?.contains(&folder));
    Ok(())
}

#[test]
fn portable_feed_references_and_renames_keep_the_same_local_identity()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    fs::write(&path, include_str!("fixtures/configuration/portable.toml"))?;
    let session = Session::open(&store, &path)?;
    let feed = store.feeds()?[0].id;
    let list = store.smart_lists()?.remove(0);
    assert_eq!(list.filter.feeds, [feed]);
    let mut config = config::decode(&fs::read_to_string(&path)?)?;
    config.smart_lists[0].name = "Renamed".into();
    fs::write(&path, config::encode(&config)?)?;
    session.flush()?;
    assert_eq!(store.smart_lists()?[0].id, list.id);
    assert_eq!(store.smart_lists()?[0].name, "Renamed");
    Ok(())
}

#[test]
fn cli_subscription_changes_publish_to_the_selected_file() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let path = directory.path().join("reader.toml");
    let result = cli(&database)
        .arg("--config")
        .arg(&path)
        .args(["add", "https://journal.example/feed"])
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let config = config::decode(&fs::read_to_string(&path)?)?;
    assert_eq!(config.feeds.len(), 1);
    assert_eq!(config.feeds[0].url, "https://journal.example/feed");
    assert!(!database.with_extension("toml").exists());
    Ok(())
}

#[test]
fn cli_validation_does_not_create_a_database_or_modify_the_document()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("must-not-exist.sqlite3");
    let path = directory.path().join("reader.toml");
    let original = "# My file\nschema_version = 1\n";
    fs::write(&path, original)?;
    let result = cli(&database)
        .args(["config", "check"])
        .arg(&path)
        .output()?;
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(!database.exists());
    assert_eq!(fs::read_to_string(&path)?, original);
    Ok(())
}

#[test]
fn recovery_keeps_both_inputs_as_backups_and_preserves_personal_data()
-> Result<()> {
    let (directory, store, _) = common::seeded_store()?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    store.create_collection("Pending folder")?;
    let invalid = "this is not valid TOML";
    fs::write(&path, invalid)?;
    assert!(session.flush().is_err());
    let backups = config::recover(&store, &path, config::Recovery::Library)?;
    assert_eq!(backups.len(), 2);
    let contents = backups
        .iter()
        .map(fs::read_to_string)
        .collect::<std::io::Result<Vec<_>>>()?;
    assert!(contents.iter().any(|text| text == invalid));
    let recovered = config::decode(&fs::read_to_string(&path)?)?;
    assert_eq!(recovered.folders[0].name, "Pending folder");
    assert!(!store.library()?.articles.is_empty());
    session.flush()?;
    Ok(())
}

#[test]
fn resubscribing_a_removed_source_republishes_its_same_portable_identity()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut config = config::decode(&fs::read_to_string(&path)?)?;
    let key = config.feeds[0].id.clone();
    config.feeds.clear();
    fs::write(&path, config::encode(&config)?)?;
    session.flush()?;
    assert!(store.feeds()?.is_empty());
    assert_eq!(store.subscribe(&feed.url)?, feed.id);
    session.flush()?;
    assert_eq!(
        config::decode(&fs::read_to_string(&path)?)?.feeds[0].id,
        key
    );
    assert!(!store.library()?.articles.is_empty());
    Ok(())
}

#[test]
fn worker_flushes_admitted_edits_even_without_notification_consumption()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let worker = session.start()?;
    for index in 0..6 {
        store.create_collection(&format!("Folder {index}"))?;
        let _ = worker.reload();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    worker.finish()?;
    assert_eq!(
        config::decode(&fs::read_to_string(&path)?)?.folders.len(),
        6
    );
    Ok(())
}

#[cfg(unix)]
#[test]
fn read_only_configuration_retains_pending_edits_until_writing_is_possible()
-> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let original = fs::read_to_string(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400))?;
    store.create_collection("Keep this pending edit")?;
    assert!(session.flush().is_err());
    assert_eq!(fs::read_to_string(&path)?, original);
    assert_eq!(
        store.catalog()?.collections[0].name,
        "Keep this pending edit"
    );
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))?;
    session.flush()?;
    assert_eq!(
        config::decode(&fs::read_to_string(&path)?)?.folders[0].name,
        "Keep this pending edit"
    );
    Ok(())
}

fn cli(database: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new(env!("CARGO_BIN_EXE_lume"));
    command
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(database);
    command
}

#[test]
fn publication_before_failed_database_ack_recovers_without_duplicate_identities()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let database = directory.path().join("library.sqlite3");
    let store = Store::open(&database)?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let folder = store.create_collection("Durable pending definition")?;
    store.subscribe("https://journal.example/feed")?;
    let failures = rusqlite::Connection::open(&database)?;
    failures.execute_batch(include_str!(
        "../src/store/sql/test_create_reject_configuration_ack.sql"
    ))?;

    assert!(session.flush().is_err());
    let published = config::decode(&fs::read_to_string(&path)?)?;
    assert_eq!(published.folders.len(), 1);
    assert_eq!(published.feeds.len(), 1);
    failures.execute_batch("DROP TRIGGER test_reject_configuration_ack")?;

    Session::open(&store, &path)?.flush()?;
    assert_eq!(config::decode(&fs::read_to_string(&path)?)?, published);
    assert_eq!(store.catalog()?.collections[0].id, folder);
    assert_eq!(store.feeds()?.len(), 1);
    Ok(())
}

#[test]
fn reading_and_publisher_updates_do_not_schedule_configuration_publication()
-> Result<()> {
    let (directory, store, feed) = common::seeded_store()?;
    let article = store.library()?.articles.remove(0);
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let original = fs::read_to_string(&path)?;
    let before =
        publication_revision(&directory.path().join("library.sqlite3"))?;

    store.save_progress(article.id, 128)?;
    store.set_starred(article.id, true)?;
    store.save_comment(article.id, None, "Private study note")?;
    let mut workspace = store.workspace()?;
    workspace.last_visit = 12345;
    workspace.article = Some(article.id);
    store.save_workspace(&workspace)?;
    let mut publisher = Store::open(&directory.path().join("library.sqlite3"))?;
    let parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    publisher.ingest(&feed, "New publisher title", &parsed.articles)?;

    assert_eq!(
        publication_revision(&directory.path().join("library.sqlite3"))?,
        before
    );
    session.flush()?;
    assert_eq!(fs::read_to_string(&path)?, original);
    Ok(())
}

fn publication_revision(path: &std::path::Path) -> Result<i64> {
    let connection = rusqlite::Connection::open(path)?;
    Ok(connection.query_row(
        "SELECT revision FROM configuration_state WHERE singleton = 1",
        [],
        |row| row.get(0),
    )?)
}

#[cfg(unix)]
#[test]
fn publication_follows_existing_symlinks_and_retains_private_permissions()
-> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let target = directory.path().join("actual.toml");
    fs::write(&target, "schema_version = 1\n")?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600))?;
    let path = directory.path().join("config.toml");
    symlink(&target, &path)?;
    let session = Session::open(&store, &path)?;
    store.create_collection("Symlink publication")?;
    session.flush()?;
    assert!(path.symlink_metadata()?.file_type().is_symlink());
    assert_eq!(config::read_document(&target)?.folders.len(), 1);
    assert_eq!(target.metadata()?.permissions().mode() & 0o777, 0o600);
    Ok(())
}

#[test]
fn file_recovery_replaces_conflicting_preferences_and_retains_both_backups()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let mut workspace = store.workspace()?;
    workspace.theme = "gruvbox".into();
    store.save_workspace(&workspace)?;
    fs::write(&path, "[ui]\ntheme = 'nord'\n")?;
    assert!(session.flush().is_err());

    let backups = config::recover(&store, &path, config::Recovery::File)?;
    assert_eq!(backups.len(), 2);
    assert_eq!(store.workspace()?.theme, "nord");
    let library = backups
        .iter()
        .find(|path| path.to_string_lossy().ends_with("-library.toml"))
        .ok_or_else(|| anyhow::anyhow!("Backup absent"))?;
    assert_eq!(config::read_document(library)?.ui.theme, "gruvbox");
    session.flush()?;
    Ok(())
}

#[test]
fn detached_sessions_cannot_reclaim_a_librarys_new_configuration_path()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let first = directory.path().join("first.toml");
    let second = directory.path().join("second.toml");
    let old = Session::open(&store, &first)?;
    Session::open(&store, &second)?;
    store.create_collection("New path owns this change")?;
    assert!(old.flush().is_err());
    assert!(config::read_document(&first)?.folders.is_empty());
    Session::open(&store, &second)?.flush()?;
    assert_eq!(config::read_document(&second)?.folders.len(), 1);
    Ok(())
}

#[test]
fn unsupported_shortcuts_are_rejected_at_the_configuration_boundary()
-> Result<()> {
    for (key, command) in [
        ("ctrl-c", "toc"),
        ("f99", "toc"),
        ("ctrl-t", "not-a-command"),
    ] {
        let text = format!("[ui.keybindings]\n'{key}' = '{command}'\n");
        assert!(config::decode(&text).is_err(), "Accepted {key}: {command}");
    }
    assert!(config::decode("[ui.keybindings]\n'ctrl-t' = 'toc'\n").is_ok());
    Ok(())
}

#[test]
fn inline_preferences_keep_untouched_literals_and_comments() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    fs::write(
        &path,
        "ui = { theme = 'nord', mouse = false } # Keep this preference\n",
    )?;
    let session = Session::open(&store, &path)?;
    let mut workspace = store.workspace()?;
    workspace.theme = "gruvbox".into();
    store.save_workspace(&workspace)?;

    session.flush()?;

    let text = fs::read_to_string(&path)?;
    assert!(text.contains("ui = {"));
    assert!(text.contains("mouse = false"));
    assert!(text.contains("# Keep this preference"));
    assert_eq!(config::decode(&text)?.ui.theme, "gruvbox");
    Ok(())
}

#[test]
fn worker_detects_file_edits_between_initial_adoption_and_start() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    fs::write(&path, "[ui]\ntheme = 'nord'\n")?;

    let worker = session.start()?;
    let update = next_update(&worker)?;

    assert_eq!(update.ui.theme, "nord");
    assert_eq!(store.workspace()?.theme, "nord");
    worker.finish()?;
    Ok(())
}

fn next_update(worker: &config::Worker) -> Result<config::Update> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if let Some(update) = worker.poll() {
            return update;
        }
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "Configuration notification timed out"
        );
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}

#[test]
fn worker_reports_a_configuration_removed_before_start() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    fs::remove_file(&path)?;
    let worker = session.start()?;

    let error = next_update(&worker)
        .err()
        .ok_or_else(|| anyhow::anyhow!("Missing file was silently accepted"))?;

    assert!(format!("{error:#}").contains("disappeared"));
    fs::write(&path, "schema_version = 1\n")?;
    worker.finish()?;
    Ok(())
}

#[cfg(unix)]
#[test]
fn a_dangling_configuration_symlink_is_never_replaced() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let target = directory.path().join("missing.toml");
    std::os::unix::fs::symlink(&target, &path)?;

    assert!(Session::open(&store, &path).is_err());
    assert_eq!(fs::read_link(&path)?, target);
    assert!(!target.exists());
    Ok(())
}
