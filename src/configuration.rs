//! CLI configuration paths and diagnostics; no implicit current-directory search.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use clap::{Subcommand, ValueEnum};
use lume::{
    config::{self, Session},
    store::Store,
};

use crate::{Cli, cli::Command};

#[derive(Subcommand)]
pub(crate) enum ConfigCommand {
    /// Print the effective configuration path without creating files.
    Path,
    /// Validate a document without opening or modifying the library.
    Check {
        /// Document to check; defaults to the effective configuration file.
        file: Option<PathBuf>,
    },
    /// Print the active document after finishing pending definition edits.
    Show,
    /// Export complete portable definitions, excluding personal reading data.
    Export {
        /// New destination; omit to print TOML to stdout.
        output: Option<PathBuf>,
    },
    /// Reconcile file and pending library edits, without collecting sources.
    Reload,
    /// Resolve a conflict, backing up both inputs before replacing definitions.
    Recover {
        /// Which input to retain as configuration authority.
        #[arg(value_enum)]
        source: Recovery,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub(crate) enum Recovery {
    File,
    Library,
}

pub(crate) fn path(cli: &Cli) -> Result<PathBuf> {
    if let Some(path) = &cli.config {
        return absolute(path.clone());
    }
    if let Some(path) = std::env::var_os("LUME_CONFIG") {
        ensure!(!path.is_empty(), "LUME_CONFIG must not be empty");
        return absolute(path.into());
    }
    if let Some(database) = &cli.database {
        return absolute(database.with_extension("toml"));
    }
    if cli.data_dir.is_some() || std::env::var_os("LUME_DATA_DIR").is_some() {
        return absolute(
            crate::data_dir::database_path(cli.data_dir.clone(), false)?
                .with_file_name("config.toml"),
        );
    }
    let directory = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .filter(|path| path.is_absolute());
    let directory = match directory {
        Some(path) => path,
        None => PathBuf::from(
            std::env::var_os("HOME")
                .filter(|home| !home.is_empty())
                .context(
                    "Set HOME, XDG_CONFIG_HOME, LUME_CONFIG or --config",
                )?,
        )
        .join(".config"),
    };
    Ok(directory.join("lume/config.toml"))
}

fn absolute(path: PathBuf) -> Result<PathBuf> {
    ensure!(
        !path.as_os_str().is_empty(),
        "Configuration path must not be empty"
    );
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(std::env::current_dir()?.join(path))
}

pub(crate) fn inspect(
    command: Option<&Command>,
    path: Option<&Path>,
    demo: bool,
) -> Result<bool> {
    let Some(Command::Config { command }) = command else {
        return Ok(false);
    };
    ensure!(!demo, "Configuration commands require a real library");
    let path = path.context("Configuration path is unavailable")?;
    match command {
        None | Some(ConfigCommand::Path) => println!("{}", path.display()),
        Some(ConfigCommand::Check { file }) => {
            let path = file.as_deref().unwrap_or(path);
            config::read_document(path)?;
            println!("Configuration valid: {}", path.display());
        }
        _ => return Ok(false),
    }
    Ok(true)
}

pub(crate) fn execute(
    command: ConfigCommand,
    store: &Store,
    path: &Path,
) -> Result<()> {
    if let ConfigCommand::Recover { source } = command {
        let source = match source {
            Recovery::File => config::Recovery::File,
            Recovery::Library => config::Recovery::Library,
        };
        for backup in config::recover(store, path, source)? {
            println!("Recovery backup: {}", backup.display());
        }
        return Ok(());
    }
    let session = Session::open(store, path)?;
    let text = &session.initial().document;
    match command {
        ConfigCommand::Show => print!("{text}"),
        ConfigCommand::Export { output } => {
            let config = config::decode(text)?;
            if let Some(path) = output {
                config::export(&config, &path)?;
            } else {
                print!("{}", config::encode(&config)?);
            }
        }
        ConfigCommand::Reload => {
            println!("Configuration reconciled: {}", session.path().display())
        }
        _ => anyhow::bail!(
            "Configuration diagnostic must run before opening the library"
        ),
    }
    Ok(())
}
