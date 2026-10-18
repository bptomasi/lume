//! CLI composition and terminal lifecycle.

mod cli;
mod terminal;

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Parser;
use lume::{demo, store::Store};

use cli::{Command, execute};

#[derive(Parser)]
#[command(
    version,
    about = "A quiet place for curious minds. Independent RSS/Atom reading."
)]
struct Cli {
    /// Explore fictional articles in a separate demonstration library.
    #[arg(long, conflicts_with = "database")]
    demo: bool,
    /// Read cached articles with network requests disabled.
    #[arg(long)]
    offline: bool,
    /// Override the local SQLite database path.
    #[arg(long)]
    database: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = cli
        .database
        .clone()
        .map_or_else(|| data_path(cli.demo), Ok)?;
    let mut store = Store::open(&path)?;
    if cli.demo {
        demo::seed(&mut store)?;
    }
    if let Some(command) = cli.command {
        return execute(command, store, path, cli.demo, cli.offline);
    }
    terminal::run(store, path, cli.demo, cli.offline)
}

fn data_path(demo: bool) -> Result<PathBuf> {
    let base = if let Some(path) = std::env::var_os("XDG_DATA_HOME") {
        PathBuf::from(path)
    } else if let Some(path) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(path)
    } else {
        let home = std::env::var_os("HOME")
            .context("Set XDG_DATA_HOME or --database")?;
        PathBuf::from(home).join(".local/share")
    };
    let name = if demo {
        "demo.sqlite3"
    } else {
        "library.sqlite3"
    };
    Ok(base.join("lume").join(name))
}
