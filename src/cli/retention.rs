//! Explicit previews and application share the background retention contract.

use anyhow::{Context, Result};
use clap::Subcommand;
use lume::{retention, store::Store};
use std::{path::Path, sync::atomic::AtomicBool};

#[derive(Subcommand)]
pub(crate) enum Command {
    /// Explain activation and scheduling without scanning article bodies.
    Status,
    /// Explain personal protections and current eligibility for an article.
    Explain { article: i64 },
    /// Count eligible content and acknowledge this exact policy without deleting.
    Preview,
    /// Apply the previously previewed policy, preserving all personal protections.
    Apply,
    /// List presets or materialize global ages, pausing automatic cleanup.
    Preset {
        /// archive, cache-only, balanced or compact; omit to compare them.
        name: Option<String>,
    },
}

pub(super) fn run(
    store: &Store,
    path: &Path,
    command: Option<Command>,
) -> Result<()> {
    let images = path
        .parent()
        .context("Library has no parent directory")?
        .join("images");
    let now = chrono::Utc::now().timestamp();
    match command.unwrap_or(Command::Status) {
        Command::Explain { article } => {
            print!("{}", store.explain_retention(article, now)?)
        }
        Command::Status => print!("{}", retention::status(store)?),
        Command::Preset { name } => {
            if let Some(name) = name {
                store.set_retention_preset(name.parse()?)?;
                println!("Preset saved to TOML; automatic retention paused.");
                print!("{}", retention::status(store)?);
            } else {
                print!("{}", lume::config::retention::presets::GUIDE);
            }
        }
        Command::Preview => {
            print!("{}", retention::preview(store, &images, now)?.text(true))
        }
        Command::Apply => print!(
            "{}",
            retention::apply(
                store,
                &images,
                now,
                &AtomicBool::new(false),
                None
            )?
            .text(false)
        ),
    }
    Ok(())
}

pub(super) fn automatic(store: &Store, path: &Path) -> Result<()> {
    let images = path
        .parent()
        .context("Library has no parent directory")?
        .join("images");
    if let Some(report) = retention::automatic(
        store,
        &images,
        chrono::Utc::now().timestamp(),
        &AtomicBool::new(false),
        None,
    )? {
        print!("{}", report.text(false));
    }
    Ok(())
}
