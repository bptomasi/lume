//! Explicit disk diagnostics share TUI accounting and never reclaim data.
use anyhow::{Context, Result};
use lume::{
    storage::{self, Unit},
    store::Store,
};
use std::path::Path;
#[derive(clap::Args)]
pub(crate) struct Arguments {
    /// Include source, SQLite object and duplicated FTS body accounting.
    #[arg(long)]
    details: bool,
    /// Output raw byte diagnostics as JSON (implies details).
    #[arg(long)]
    json: bool,
    /// auto, bytes, MiB or GiB.
    #[arg(long, default_value = "auto")]
    unit: String,
}

pub(super) fn run(
    store: &Store,
    path: &Path,
    arguments: Arguments,
) -> Result<()> {
    let Arguments {
        details,
        json,
        unit,
    } = arguments;
    let unit: Unit = unit.parse()?;
    if details || json {
        let details = storage::details::inspect(store)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&details)?);
        } else {
            print!("{}", details.text(unit));
        }
    } else {
        let images = path.parent().context("Library directory")?.join("images");
        print!("{}", storage::inspect_with_unit(store, &images, unit)?);
    }
    Ok(())
}
