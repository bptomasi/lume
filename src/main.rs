//! CLI composition and terminal lifecycle.

mod cli;
mod data_dir;
mod terminal;

use std::path::PathBuf;

use anyhow::Result;
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
    /// Override LUME_DATA_DIR and the default ~/.local/lume directory.
    #[arg(long, conflicts_with = "database")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let path = if let Some(path) = cli.database {
        path
    } else {
        data_dir::database_path(cli.data_dir, cli.demo)?
    };
    let mut store = Store::open(&path)?;
    if cli.demo {
        demo::seed(&mut store)?;
    }
    if let Some(command) = cli.command {
        return execute(command, store, path, cli.demo, cli.offline);
    }
    terminal::run(store, path, cli.demo, cli.offline)
}
