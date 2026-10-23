//! Retention is explicit, works offline and reconciles its policy with TOML.

use anyhow::Result;
use lume::{config, model::View, store::Store};
use std::{
    path::Path,
    process::{Command, Output},
};

fn run(root: &Path, arguments: &[&str]) -> Result<Output> {
    Ok(Command::new(env!("CARGO_BIN_EXE_lume"))
        .args(["--offline", "--data-dir"])
        .arg(root.join("data"))
        .arg("--config")
        .arg(root.join("config.toml"))
        .args(arguments)
        .output()?)
}

#[test]
fn retention_requires_preview_applies_limits_and_rejects_changed_policies()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let path = root.join("data/library.sqlite3");
    let store = Store::open(&path)?;
    let seed = rusqlite::Connection::open(&path)?;
    seed.execute_batch(include_str!(
        "../src/store/sql/test_setup_retention.sql"
    ))?;
    drop(seed);
    let session = config::Session::open(&store, &root.join("config.toml"))?;
    let mut policy = config::decode(&session.initial().document)?;
    policy.retention.defaults.read_days = config::retention::Age(Some(90));
    policy.retention.max_articles_per_run = 2;
    std::fs::write(root.join("config.toml"), config::encode(&policy)?)?;
    assert!(!run(root, &["retention", "apply"])?.status.success());
    let preview = run(root, &["retention", "preview"])?;
    assert!(
        preview.status.success(),
        "{}",
        String::from_utf8_lossy(&preview.stderr)
    );
    assert!(String::from_utf8(preview.stdout)?.contains("Articles: 24"));
    assert_eq!(store.view_count(&View::All)?, 24);
    let applied = run(root, &["retention", "apply"])?;
    assert!(
        applied.status.success(),
        "{}",
        String::from_utf8_lossy(&applied.stderr)
    );
    assert_eq!(store.view_count(&View::All)?, 22);
    policy.retention.defaults.read_days = config::retention::Age(Some(30));
    std::fs::write(root.join("config.toml"), config::encode(&policy)?)?;
    assert!(!run(root, &["retention", "apply"])?.status.success());
    assert_eq!(store.view_count(&View::All)?, 22);
    Ok(())
}

#[test]
fn inline_credentials_are_hidden_from_cli_and_kept_in_private_configuration()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let text = r#"schema_version = 1
[[servers]]
id = "home"
url = "https://reader.example"
token = "private-local-key"
"#;
    let path = root.join("config.toml");
    std::fs::write(&path, text)?;
    for arguments in
        [&["config", "show"][..], &["config", "export"], &["servers"]]
    {
        let result = run(root, arguments)?;
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(
            !String::from_utf8(result.stdout)?.contains("private-local-key")
        );
    }
    assert!(std::fs::read_to_string(&path)?.contains("private-local-key"));
    let explicit = run(root, &["config", "show", "--include-secrets"])?;
    assert!(explicit.status.success());
    assert!(String::from_utf8(explicit.stdout)?.contains("private-local-key"));
    Ok(())
}
