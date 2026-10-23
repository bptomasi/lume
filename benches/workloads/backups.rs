//! Real WAL copies, checksums and isolated restores qualify managed rotation.
use anyhow::{Context, Result, ensure};
use lume::{
    backups,
    config::{self, backups::Backups},
    model::View,
    store::Store,
};
use std::sync::atomic::AtomicBool;

pub fn run(iterations: usize, stress: bool) -> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path();
    let mut store = Store::open(&root.join("library.sqlite3"))?;
    let count = if stress { 10_000 } else { 1000 };
    super::seed(&mut store, count)?;
    let path = root.join("config.toml");
    let session = config::Session::open(&store, &path)?;
    let mut document = config::decode(&session.initial().document)?;
    document.backups = Backups {
        keep: 2,
        ..Default::default()
    };
    std::fs::write(&path, config::encode(&document)?)?;
    config::Session::open(&store, &path)?;
    let mut round = 0;
    super::measure(
        &format!("checked backup + isolated restore ({count} articles)"),
        iterations,
        || {
            round += 1;
            let snapshot =
                backups::run(&store, round, &AtomicBool::new(false))?
                    .context("Missing backup")?;
            let output = root.join(format!("restored-{round}"));
            backups::restore(&snapshot, &output)?;
            let restored = Store::open(&output.join("library.sqlite3"))?;
            ensure!(
                restored.view_count(&View::All)? == count,
                "Snapshot lost articles"
            );
            drop(restored);
            std::fs::remove_dir_all(output)?;
            Ok(())
        },
    )
}
