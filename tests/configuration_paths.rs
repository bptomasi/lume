//! Actual CLI path precedence never touches a personal configuration file.

use std::{path::Path, process::Command};

use anyhow::{Result, ensure};

#[test]
fn cli_paths_obey_explicit_environment_and_library_precedence() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let database = root.join("library.sqlite3");
    let environment = root.join("environment.toml");
    let flag = root.join("flag.toml");
    let output = cli(root)
        .env("LUME_CONFIG", &environment)
        .arg("--database")
        .arg(&database)
        .arg("--config")
        .arg(&flag)
        .args(["config", "path"])
        .output()?;
    assert_path(&output, &flag)?;
    let output = cli(root)
        .env("LUME_CONFIG", &environment)
        .arg("--database")
        .arg(&database)
        .args(["config", "path"])
        .output()?;
    assert_path(&output, &environment)?;
    let output = cli(root)
        .arg("--database")
        .arg(&database)
        .args(["config", "path"])
        .output()?;
    assert_path(&output, &database.with_extension("toml"))?;
    assert!(!database.exists());
    assert!(!flag.exists());
    Ok(())
}

#[test]
fn default_and_custom_data_directories_keep_configuration_isolated()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let output = cli(root).args(["config", "path"]).output()?;
    assert_path(&output, &root.join("xdg/lume/config.toml"))?;
    for argument in ["--data-dir", "environment"] {
        let mut command = cli(root);
        if argument == "environment" {
            command.env("LUME_DATA_DIR", root.join("data"));
        } else {
            command.arg(argument).arg(root.join("data"));
        }
        let output = command.args(["config", "path"]).output()?;
        assert_path(&output, &root.join("data/config.toml"))?;
    }
    assert!(!root.join("xdg").exists());
    assert!(!root.join("data").exists());
    Ok(())
}

#[test]
fn invalid_environment_and_oversized_documents_fail_without_a_database()
-> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let output = cli(root)
        .env("LUME_CONFIG", "")
        .args(["config", "path"])
        .output()?;
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("must not be empty")
    );
    let document = root.join("large.toml");
    std::fs::write(&document, "#".repeat(2 * 1024 * 1024 + 1))?;
    let output = cli(root)
        .args(["config", "check"])
        .arg(&document)
        .output()?;
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("2 MiB"));
    assert!(!root.join("data").exists());
    Ok(())
}

fn cli(root: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lume"));
    command
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR")
        .env("HOME", root)
        .env("XDG_CONFIG_HOME", root.join("xdg"));
    command
}

fn assert_path(output: &std::process::Output, expected: &Path) -> Result<()> {
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        expected.to_string_lossy()
    );
    Ok(())
}
