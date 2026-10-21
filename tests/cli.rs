//! CLI scenarios verify durable effects and isolation through the real binary.

use std::process::Command;

use anyhow::Result;
use lume::store::Store;

#[test]
fn offline_cli_rejects_all_network_commands_before_attempting_io() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("offline.sqlite3");
    for args in [
        vec!["fetch", "42"],
        vec!["sync"],
        vec!["discover", "https://journal.example/"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_lume"))
            .env_remove("LUME_CONFIG")
            .env_remove("LUME_DATA_DIR")
            .arg("--database")
            .arg(&path)
            .arg("--offline")
            .args(args)
            .output()?;
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("Network disabled by --offline")
        );
    }
    assert!(Store::open(&path)?.library()?.articles.is_empty());
    Ok(())
}

#[test]
fn add_and_list_normalize_subscriptions_without_network_access() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let added = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(&path)
        .args(["add", "https://journal.example/feed#fragment"])
        .output()?;
    assert!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );
    let listed = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(&path)
        .arg("feeds")
        .output()?;
    assert!(listed.status.success());
    let text = String::from_utf8(listed.stdout)?;
    assert!(text.contains("https://journal.example/feed"));
    assert!(!text.contains("#fragment"));
    assert_eq!(Store::open(&path)?.feeds()?.len(), 1);
    Ok(())
}

#[test]
fn demo_cannot_use_a_custom_real_database() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("existing.sqlite3");
    std::fs::write(&path, b"existing user data")?;
    let rejected = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .args(["--demo", "--database"])
        .arg(&path)
        .output()?;
    assert!(!rejected.status.success());
    assert_eq!(std::fs::read(&path)?, b"existing user data");
    Ok(())
}

#[test]
fn cli_rejects_credentials_without_echoing_the_secret() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let rejected = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(&path)
        .args(["add", "https://user:supersecret@journal.example/feed"])
        .output()?;
    assert!(!rejected.status.success());
    assert!(!String::from_utf8_lossy(&rejected.stderr).contains("supersecret"));
    assert!(Store::open(&path)?.feeds()?.is_empty());
    Ok(())
}

#[test]
fn non_interactive_tui_fails_with_an_actionable_message() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let result = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(directory.path().join("library.sqlite3"))
        .arg("--offline")
        .output()?;
    assert!(!result.status.success());
    assert!(
        String::from_utf8_lossy(&result.stderr)
            .contains("interactive terminal")
    );
    Ok(())
}

#[test]
fn cli_import_export_and_search_use_the_real_library_contracts() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let import = directory.path().join("feeds.opml");
    std::fs::write(
        &import,
        r#"
        <opml version="2.0"><body>
            <outline text="Journal" xmlUrl="https://journal.example/feed"/>
        </body></opml>
    "#,
    )?;
    run(&path, &["import", import.to_str().unwrap_or_default()])?;
    let exported = directory.path().join("export.opml");
    run(
        &path,
        &["export-opml", exported.to_str().unwrap_or_default()],
    )?;
    assert!(
        std::fs::read_to_string(exported)?.contains("journal.example/feed")
    );
    let mut store = Store::open(&path)?;
    let feed = store.feeds()?.remove(0);
    let parsed =
        lume::source::parse(include_bytes!("fixtures/rss.xml"), &feed.url)?;
    store.ingest(&feed, &parsed.title, &parsed.articles)?;

    let found = run(&path, &["search", "measure"])?;

    assert!(String::from_utf8(found.stdout)?.contains("Cache & consequence"));
    let note = directory.path().join("article.md");
    let id = store.library()?.articles[0].id.to_string();
    run(&path, &["export", &id, note.to_str().unwrap_or_default()])?;
    assert!(std::fs::read_to_string(note)?.contains("Keep **one** copy"));
    Ok(())
}

fn run(
    path: &std::path::Path,
    arguments: &[&str],
) -> Result<std::process::Output> {
    let output = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(path)
        .args(arguments)
        .output()?;
    anyhow::ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(output)
}
