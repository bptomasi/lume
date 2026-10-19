//! Real CLI processes use one configured library across working directories.

use std::{path::Path, process::Command};

use anyhow::{Result, ensure};

fn cli(home: &Path, working: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lume"));
    command
        .env("HOME", home)
        .env_remove("LUME_DATA_DIR")
        .current_dir(working);
    command
}

#[test]
fn default_library_is_shared_across_directories_and_ignores_legacy_xdg()
-> Result<()> {
    let temporary = tempfile::tempdir()?;
    let home = temporary.path().join("home");
    let first = temporary.path().join("first");
    let second = temporary.path().join("second");
    std::fs::create_dir_all(&first)?;
    std::fs::create_dir_all(&second)?;
    let added = cli(&home, &first)
        .env("XDG_DATA_HOME", temporary.path().join("legacy"))
        .args(["add", "https://journal.example/rss"])
        .output()?;
    ensure!(
        added.status.success(),
        "{}",
        String::from_utf8_lossy(&added.stderr)
    );

    let listed = cli(&home, &second).arg("feeds").output()?;

    ensure!(listed.status.success(), "Listing failed");
    assert!(
        String::from_utf8_lossy(&listed.stdout).contains("journal.example")
    );
    assert!(home.join(".local/lume/library.sqlite3").is_file());
    assert!(!first.join("library.sqlite3").exists());
    assert!(!second.join("library.sqlite3").exists());
    assert!(!temporary.path().join("legacy/lume").exists());
    Ok(())
}

#[test]
fn environment_selects_a_directory_without_requiring_home() -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let directory = temporary.path().join("custom library");
    let output = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env_remove("HOME")
        .env("LUME_DATA_DIR", &directory)
        .args(["add", "https://journal.example/rss"])
        .output()?;
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(directory.join("library.sqlite3").is_file());
    Ok(())
}

#[test]
fn directory_flag_overrides_environment_and_keeps_demo_separate() -> Result<()>
{
    let temporary = tempfile::tempdir()?;
    let ignored = temporary.path().join("environment");
    let chosen = temporary.path().join("chosen directory");
    for demo in [false, true] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lume"));
        command
            .env("LUME_DATA_DIR", &ignored)
            .arg("--data-dir")
            .arg(&chosen);
        if demo {
            command.arg("--demo");
        }
        let output = command.arg("feeds").output()?;
        ensure!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    assert!(chosen.join("library.sqlite3").is_file());
    assert!(chosen.join("demo.sqlite3").is_file());
    assert!(!ignored.exists());
    Ok(())
}

#[test]
fn explicit_database_wins_over_environment_and_conflicts_with_directory_flag()
-> Result<()> {
    let temporary = tempfile::tempdir()?;
    let ignored = temporary.path().join("ignored");
    let database = temporary.path().join("chosen.sqlite3");
    let output = Command::new(env!("CARGO_BIN_EXE_lume"))
        .env("LUME_DATA_DIR", &ignored)
        .arg("--database")
        .arg(&database)
        .arg("feeds")
        .output()?;
    ensure!(output.status.success(), "Explicit database failed");
    assert!(database.is_file());
    assert!(!ignored.exists());

    let rejected = Command::new(env!("CARGO_BIN_EXE_lume"))
        .arg("--database")
        .arg(&database)
        .arg("--data-dir")
        .arg(&ignored)
        .arg("feeds")
        .output()?;
    assert!(!rejected.status.success());
    assert!(!ignored.exists());
    Ok(())
}
