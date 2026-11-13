//! Pure configuration command arguments, shared by parsing and execution.

use clap::{Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Subcommand)]
pub(crate) enum ConfigCommand {
    /// Print the effective configuration path without creating files.
    Path,
    /// Validate a document without opening or modifying the library.
    Check {
        /// Document to check; defaults to the effective configuration file.
        file: Option<PathBuf>,
    },
    /// Format an existing TOML file without opening or modifying the library.
    Format {
        /// Document to format; defaults to the effective configuration file.
        file: Option<PathBuf>,
    },
    /// Print the active document after finishing pending definition edits.
    Show {
        /// Explicitly include inline credentials in the displayed document.
        #[arg(long)]
        include_secrets: bool,
    },
    /// Export complete portable definitions, excluding personal reading data.
    Export {
        /// New destination; omit to print TOML to stdout.
        output: Option<PathBuf>,
        /// Explicitly include inline credentials in the exported definitions.
        #[arg(long)]
        include_secrets: bool,
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
