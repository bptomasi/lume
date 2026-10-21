//! Human-readable generated configuration and explicit comment-safe formatting.

use std::{fs, process::Command};

use anyhow::{Result, ensure};
use lume::{
    config::{self, Session},
    store::Store,
};

const COMPACT: &str = r#"# Reader settings
schema_version=1

# Subscriptions
feeds=[{id="journal",url='https://journal.example/feed',enabled=true}]
smart_lists=[{id="systems",name="Systems",query="consensus",feeds=["journal"]}]
recommendations={interests=[{name="Systems",weight=8,query='raft OR consensus'}]}

[ui]
theme = 'nord' # Keep the chosen palette
"#;

#[test]
fn explicit_format_expands_records_and_preserves_comments_and_values()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("config.toml");
    fs::write(&path, COMPACT)?;
    let before = config::read_document(&path)?;

    config::format_file(&path)?;

    let text = fs::read_to_string(&path)?;
    assert!(text.contains("schema_version = 1"));
    for header in [
        "[[feeds]]",
        "[[smart_lists]]",
        "[[recommendations.interests]]",
    ] {
        assert!(text.contains(header), "Missing {header}: {text}");
    }
    assert!(text.contains("# Reader settings"));
    assert!(text.contains("# Subscriptions"));
    assert!(text.contains("theme = 'nord' # Keep the chosen palette"));
    assert!(text.contains("url = 'https://journal.example/feed'"));
    assert_eq!(config::read_document(&path)?, before);
    assert!(text.lines().all(|line| line.chars().count() <= 80));
    config::format_file(&path)?;
    assert_eq!(fs::read_to_string(&path)?, text, "Format is not idempotent");
    Ok(())
}

#[test]
fn generated_sources_stay_in_table_blocks_after_edits() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let path = directory.path().join("config.toml");
    let session = Session::open(&store, &path)?;
    let feed = store.subscribe("https://journal.example/feed")?;
    store.subscribe("https://other.example/feed")?;
    session.flush()?;
    assert_eq!(fs::read_to_string(&path)?.matches("[[feeds]]").count(), 2);
    store.unsubscribe(feed)?;
    session.flush()?;
    let text = fs::read_to_string(&path)?;
    assert_eq!(text.matches("[[feeds]]").count(), 2);
    assert!(!text.contains("feeds = [{"));
    assert!(text.contains("enabled = false"));
    Ok(())
}

#[test]
fn formatting_cli_never_creates_a_database() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("config.toml");
    let database = directory.path().join("must-not-exist.sqlite3");
    fs::write(&path, COMPACT)?;

    let output = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .arg("--database")
        .arg(&database)
        .args(["config", "format"])
        .arg(&path)
        .output()?;

    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!database.exists());
    assert!(fs::read_to_string(&path)?.contains("[[feeds]]"));
    Ok(())
}

#[test]
fn invalid_input_is_retained_without_partial_formatting() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("config.toml");
    for invalid in ["unknown = true\n", "[[feeds]]\nid = 'incomplete'\n"] {
        fs::write(&path, invalid)?;
        assert!(config::format_file(&path).is_err());
        assert_eq!(fs::read_to_string(&path)?, invalid);
    }
    Ok(())
}

#[test]
fn long_arrays_wrap_without_confusing_quoted_hashes_with_comments() -> Result<()>
{
    let mut document = config::decode(COMPACT)?;
    let id = "publication_with_a_long_but_valid_portable_subscription_identity";
    document.feeds[0].id = id.into();
    document.smart_lists[0].feeds = vec![id.into()];
    document.folders.push(config::Folder {
        id: "notes".into(),
        name: "Notes #1".into(),
        query: String::new(),
    });

    let text = config::encode(&document)?;

    assert!(text.contains(&format!("feeds = [\n    \"{id}\",\n]")));
    assert!(text.contains("[[folders]]\nid = \"notes\"\nname = \"Notes #1\""));
    assert_eq!(config::decode(&text)?, document);
    assert!(text.lines().all(|line| line.chars().count() <= 80));
    Ok(())
}

#[test]
fn annotated_inline_records_retain_their_comments_and_layout() -> Result<()> {
    let text = "schema_version=1\nfeeds = [ # A carefully annotated group\n\
        {id='journal',url='https://journal.example/feed'}, # Publisher\n]\n";
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("config.toml");
    fs::write(&path, text)?;

    config::format_file(&path)?;

    let formatted = fs::read_to_string(&path)?;
    assert!(formatted.contains("feeds = [ # A carefully annotated group"));
    assert!(formatted.contains(
        "{id='journal',url='https://journal.example/feed'}, # Publisher"
    ));
    assert_eq!(config::decode(&formatted)?, config::decode(text)?);
    config::format_file(&path)?;
    assert_eq!(fs::read_to_string(&path)?, formatted);

    let store = Store::open(&directory.path().join("library.sqlite3"))?;
    let session = Session::open(&store, &path)?;
    store.unsubscribe(store.feeds()?[0].id)?;
    session.flush()?;
    let updated = fs::read_to_string(&path)?;
    assert!(updated.contains("# A carefully annotated group"));
    assert!(updated.contains("# Publisher"));
    assert!(!config::read_document(&path)?.feeds[0].enabled);
    Ok(())
}

#[cfg(unix)]
#[test]
fn formatting_retains_symlinks_permissions_and_read_only_input() -> Result<()> {
    use std::os::unix::fs::{PermissionsExt, symlink};

    let directory = tempfile::tempdir()?;
    let target = directory.path().join("target.toml");
    let path = directory.path().join("config.toml");
    fs::write(&target, COMPACT)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600))?;
    symlink(&target, &path)?;

    config::format_file(&path)?;

    assert!(path.symlink_metadata()?.file_type().is_symlink());
    assert_eq!(target.metadata()?.permissions().mode() & 0o777, 0o600);
    assert!(fs::read_to_string(&target)?.contains("[[feeds]]"));
    fs::write(&target, COMPACT)?;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o400))?;
    assert!(config::format_file(&path).is_err());
    assert_eq!(fs::read_to_string(&target)?, COMPACT);
    Ok(())
}
