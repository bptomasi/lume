//! CLI composition and terminal lifecycle.

mod cli;
mod configuration;
mod data_dir;
mod terminal;

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use lume::{demo, store::Store};

use cli::{Command, execute};

#[derive(Parser)]
#[command(
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("LUME_BUILD_REVISION"), ")"),
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
    /// Override LUME_CONFIG and the XDG TOML configuration path.
    #[arg(long, conflicts_with = "demo")]
    config: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    if let Some(Command::Restore { snapshot, output }) = &cli.command {
        Store::restore_snapshot(snapshot, output)?;
        println!(
            "Restored. Open with --data-dir {} --config {}",
            output.display(),
            output.join("config.toml").display()
        );
        return Ok(());
    }
    if let Some(Command::Backup { output }) = &cli.command {
        Store::snapshot_existing(&library_path(&cli)?, output)?;
        println!("Snapshot saved: {}", output.display());
        return Ok(());
    }
    let config_path = if cli.demo {
        None
    } else {
        Some(configuration::path(&cli)?)
    };
    if configuration::inspect(
        cli.command.as_ref(),
        config_path.as_deref(),
        cli.demo,
    )? {
        return Ok(());
    }
    let (path, store) = open_library(&cli)?;
    run(cli, path, store, config_path)
}

fn open_library(cli: &Cli) -> Result<(PathBuf, Store)> {
    let path = library_path(cli)?;
    let mut store = Store::open(&path)?;
    if cli.demo {
        demo::seed(&mut store)?;
    }
    Ok((path, store))
}

fn library_path(cli: &Cli) -> Result<PathBuf> {
    if let Some(path) = &cli.database {
        return Ok(path.clone());
    }
    Ok(data_dir::database_path(cli.data_dir.clone(), cli.demo)?)
}

fn run(
    cli: Cli,
    path: PathBuf,
    store: Store,
    config_path: Option<PathBuf>,
) -> Result<()> {
    if let Some(Command::Config {
        command: Some(command),
    }) = cli.command
    {
        return configuration::execute(
            command,
            &store,
            config_path.as_deref().ok_or_else(|| {
                anyhow::anyhow!("Configuration commands require a real library")
            })?,
        );
    }
    let session = config_path
        .as_deref()
        .map(|path| lume::config::Session::open(&store, path))
        .transpose()?;
    if let Some(command) = cli.command {
        let result = execute(command, store, path, cli.demo, cli.offline);
        let publish = session
            .as_ref()
            .map(lume::config::Session::flush)
            .transpose();
        return result.and(publish.map(|_| ()));
    }
    terminal::run(store, path, cli.demo, cli.offline, session)
}
