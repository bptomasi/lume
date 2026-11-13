//! CLI configuration paths and diagnostics; no implicit current-directory search.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use lume::{config, configuration_service::Session, store::Store};

use crate::configuration_arguments::{ConfigCommand, Recovery};

pub(crate) fn path(
    config: Option<&Path>,
    database: Option<&Path>,
    data_dir: Option<&Path>,
) -> Result<PathBuf> {
    if let Some(path) = config {
        return absolute(path.to_path_buf());
    }
    if let Some(path) = std::env::var_os("LUME_CONFIG") {
        ensure!(!path.is_empty(), "LUME_CONFIG must not be empty");
        return absolute(path.into());
    }
    if let Some(database) = database {
        return absolute(database.with_extension("toml"));
    }
    if data_dir.is_some() || std::env::var_os("LUME_DATA_DIR").is_some() {
        return absolute(
            crate::data_dir::database_path(
                data_dir.map(Path::to_path_buf),
                false,
            )?
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
    command: Option<&ConfigCommand>,
    path: &Path,
) -> Result<bool> {
    match command {
        None | Some(ConfigCommand::Path) => println!("{}", path.display()),
        Some(ConfigCommand::Check { file }) => {
            let path = file.as_deref().unwrap_or(path);
            config::read_document(path)?;
            println!("Configuration valid: {}", path.display());
        }
        Some(ConfigCommand::Format { file }) => {
            let path = file.as_deref().unwrap_or(path);
            config::format_file(path)?;
            println!("Configuration formatted: {}", path.display());
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
        return recover(store, path, source);
    }
    let session = Session::open(store, path)?;
    let text = &session.initial().document;
    match command {
        ConfigCommand::Show { include_secrets } => {
            if include_secrets {
                print!("{text}");
            } else {
                print!("{}", config::redacted_document(text)?);
            }
        }
        ConfigCommand::Export {
            output,
            include_secrets,
        } => {
            let mut config = config::decode(text)?;
            if !include_secrets {
                config = config.redacted();
            }
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

fn recover(store: &Store, path: &Path, source: Recovery) -> Result<()> {
    let source = match source {
        Recovery::File => config::Recovery::File,
        Recovery::Library => config::Recovery::Library,
    };
    for backup in lume::configuration_service::recover(store, path, source)? {
        println!("Recovery backup: {}", backup.display());
    }
    Ok(())
}
