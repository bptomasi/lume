//! Noninteractive server management shares the same declarative store edits.

use anyhow::Result;
use clap::Subcommand;
use lume::store::Store;

#[derive(Subcommand)]
pub(crate) enum Edit {
    /// Connect a Miniflux server; credentials stay in the named environment variable.
    Add {
        /// Absolute HTTP(S) Miniflux root URL.
        url: String,
        /// Name of the environment variable holding the API token.
        token_env: String,
    },
    /// Remove the connection while retaining cached articles and notes.
    Remove { id: i64 },
    /// Resume collection for a paused connection.
    Enable { id: i64 },
    /// Pause collection without hiding cached articles.
    Disable { id: i64 },
}

pub(super) fn edit(store: &Store, command: Edit) -> Result<()> {
    match command {
        Edit::Add { url, token_env } => {
            let id = store.add_server(&url, &token_env)?;
            println!("Server {id} saved. Run lume sync or press r.");
        }
        Edit::Remove { id } => {
            store.remove_server(id)?;
            println!("Server {id} removed; cached articles and notes kept.");
        }
        Edit::Enable { id } => store.set_server_enabled(id, true)?,
        Edit::Disable { id } => store.set_server_enabled(id, false)?,
    }
    Ok(())
}

pub(super) fn print(store: &Store) -> Result<()> {
    let servers = store.servers()?;
    if servers.is_empty() {
        println!("No servers. Use lume server add URL TOKEN_ENV.");
    }
    for server in servers {
        println!(
            "{}\t{}\t{}\t{}",
            server.id,
            if server.enabled { "enabled" } else { "paused" },
            server.url,
            if server.token.is_some() {
                "TOML (hidden)"
            } else {
                server.token_env.as_str()
            }
        );
        if !server.error.is_empty() {
            println!("  {}", lume::content::label(&server.error, 500));
        }
    }
    Ok(())
}
