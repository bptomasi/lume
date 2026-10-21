//! CLI ordering never renumbers subscriptions or changes cached reading data.

use std::{path::Path, process::Command};

use anyhow::{Result, ensure};
use lume::{source::discovery::Candidate, store::Store};

#[test]
fn subscriptions_have_labeled_ids_and_explicit_title_or_id_order() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let store = seed(&path)?;

    let alphabetical = listing(&path, &[])?;
    let by_id = listing(&path, &["--sort", "id"])?;

    assert_eq!(
        alphabetical
            .lines()
            .next()
            .unwrap_or("")
            .split_whitespace()
            .collect::<Vec<_>>(),
        ["ID", "TITLE", "URL"]
    );
    assert_eq!(ids(&alphabetical), [2, 3, 1]);
    assert_eq!(ids(&by_id), [1, 2, 3]);
    let actual = store.feeds()?;
    assert_eq!(actual[0].id, 2);
    assert_eq!(actual[1].id, 3);
    assert_eq!(actual[2].id, 1);
    Ok(())
}

#[test]
fn error_filter_preserves_ids_and_cannot_emit_terminal_controls() -> Result<()>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let store = seed(&path)?;
    store.feed_error(3, "Feed exceeds 4 MiB\n\u{1b}[31m")?;

    let output = listing(&path, &["--errors", "--sort", "id"])?;

    assert_eq!(ids(&output), [3]);
    assert!(output.contains("Last error: Feed exceeds 4 MiB"));
    assert!(!output.contains('\u{1b}'));
    assert_eq!(store.feeds()?.len(), 3);
    Ok(())
}

#[test]
fn empty_library_has_a_header_and_invalid_order_is_an_error() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("library.sqlite3");
    let output = listing(&path, &["--errors"])?;
    assert_eq!(output.lines().count(), 1);
    assert!(ids(&output).is_empty());

    let rejected = command(&path).args(["--sort", "unknown"]).output()?;
    assert!(!rejected.status.success());
    assert!(
        String::from_utf8_lossy(&rejected.stderr).contains("invalid value")
    );
    Ok(())
}

fn seed(path: &Path) -> Result<Store> {
    let store = Store::open(path)?;
    store.import_feeds(&[
        Candidate {
            title: "Zulu".into(),
            url: "https://zulu.example/feed".into(),
        },
        Candidate {
            title: "alpha".into(),
            url: "https://alpha.example/feed".into(),
        },
        Candidate {
            title: "Alpha".into(),
            url: "https://other.example/feed".into(),
        },
    ])?;
    Ok(store)
}

fn command(path: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lume"));
    command
        .env_remove("LUME_CONFIG")
        .env_remove("LUME_DATA_DIR");
    command.arg("--database").arg(path).arg("feeds");
    command
}

fn listing(path: &Path, arguments: &[&str]) -> Result<String> {
    let output = command(path).args(arguments).output()?;
    ensure!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    Ok(String::from_utf8(output.stdout)?)
}

fn ids(output: &str) -> Vec<i64> {
    output
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next()?.parse().ok())
        .collect()
}
