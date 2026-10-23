//! Noninteractive commands use the same persistence and exchange contracts.

mod feeds;
mod retention;
mod servers;
mod storage;

use std::path::{Path, PathBuf};

use anyhow::{Result, bail};
use clap::Subcommand;
use lume::{
    exchange,
    model::{ArticleQuery, View},
    source,
    store::Store,
};

/// Local and network operations available without opening the terminal UI.
#[derive(Subcommand)]
pub(super) enum Command {
    /// Subscribe to an explicit RSS/Atom/JSON Feed.
    Add {
        /// Absolute HTTP(S) feed URL.
        url: String,
    },
    /// Install curated computing feeds and missing topic searches, without HTTP.
    #[command(hide = true)]
    Starter,
    /// Install an optional bundled reading preset, preserving personal data.
    Preset {
        /// Available preset name.
        #[arg(value_parser = ["engineering"])]
        name: String,
    },
    /// Inspect, validate, export or recover declarative configuration.
    Config {
        #[command(subcommand)]
        command: Option<crate::configuration::ConfigCommand>,
    },
    /// List subscriptions by title, with permanent IDs and collection errors.
    Feeds {
        /// Display order; IDs remain stable regardless of the chosen order.
        #[arg(long, value_enum, default_value_t = feeds::Sort::Title)]
        sort: feeds::Sort,
        /// Show only subscriptions with a recorded collection error.
        #[arg(long)]
        errors: bool,
    },
    /// List configured remote servers, status and token variable names.
    Servers,
    /// Preview or apply explicitly configured cache and article lifetimes.
    Retention {
        #[command(subcommand)]
        command: Option<retention::Command>,
    },
    /// Add, remove, pause or resume a remote reader connection.
    Server {
        #[command(subcommand)]
        command: servers::Edit,
    },
    /// Collect every active subscription.
    Sync,
    /// Retrieve and cache an article's full text from its original URL.
    Fetch {
        /// Durable article ID shown by search.
        article: i64,
    },
    /// Discover advertised feeds; print choices without subscribing.
    Discover {
        /// Website or explicit feed URL.
        url: String,
    },
    /// Import OPML subscriptions atomically.
    Import {
        /// Existing OPML file.
        path: PathBuf,
    },
    /// Export active subscriptions as OPML.
    ExportOpml {
        /// New output path; existing files are never overwritten.
        output: PathBuf,
    },
    /// Export one article and its notes as Markdown.
    Export {
        /// Durable article ID shown by search.
        article: i64,
        /// New Markdown file.
        output: PathBuf,
    },
    /// Export every annotation and its source context.
    ExportNotebook {
        /// New Markdown file.
        output: PathBuf,
    },
    /// Reconcile annotated articles into a managed Markdown directory.
    ExportNotes {
        /// Managed Markdown/Obsidian directory; local edits are preserved.
        directory: PathBuf,
    },
    /// Search all cached article bodies and annotations.
    Search {
        /// Boolean query with literal terms, phrases and supported field filters.
        text: String,
        /// Maximum number of rows to print (1–500).
        #[arg(long, default_value_t = 50, value_parser = clap::value_parser!(u16).range(1..=500))]
        limit: u16,
    },
    /// Inspect disk usage or explicit source/table/FTS diagnostics.
    Storage(storage::Arguments),
    /// Snapshot committed articles, annotations and declarative preferences.
    Backup {
        /// New SQLite snapshot file; never overwrites an existing file.
        output: PathBuf,
    },
    /// Validate and restore a snapshot into a new isolated directory.
    Restore {
        /// Existing SQLite snapshot, opened read-only.
        snapshot: PathBuf,
        /// New directory receiving library.sqlite3 and config.toml.
        output: PathBuf,
    },
    /// Undo the last personal edit.
    Undo,
}

pub(super) fn execute(
    command: Command,
    store: Store,
    path: PathBuf,
    demo: bool,
    offline: bool,
) -> Result<()> {
    match command {
        Command::Add { url } => {
            real_library(demo)?;
            let id = store.subscribe(&url)?;
            println!("Subscription {id} saved. Run lume sync or press r.");
        }
        Command::Starter | Command::Preset { .. } => {
            real_library(demo)?;
            install_reading(&store)?;
        }
        Command::Feeds { sort, errors } => feeds::print(&store, sort, errors)?,
        Command::Servers => servers::print(&store)?,
        Command::Storage(arguments) => storage::run(&store, &path, arguments)?,
        Command::Retention { command } => {
            real_library(demo)?;
            retention::run(&store, &path, command)?;
        }
        Command::Server { command } => {
            real_library(demo)?;
            servers::edit(&store, command)?;
        }
        Command::Sync => {
            real_library(demo)?;
            network_enabled(offline)?;
            sync_subscriptions(&path)?;
            retention::automatic(&store, &path)?;
        }
        Command::Fetch { article } => {
            real_library(demo)?;
            network_enabled(offline)?;
            source::article::fetch_once(&path, article)?;
            println!("Full article {article} cached for offline reading.");
        }
        Command::Discover { url } => discover(&url, offline)?,
        Command::Search { text, limit } => search(&store, text, limit)?,
        Command::Undo => undo_change(&store)?,
        other => exchange_command(other, &store)?,
    }
    Ok(())
}

fn exchange_command(command: Command, store: &Store) -> Result<()> {
    match command {
        Command::Import { path } => {
            let candidates =
                exchange::opml::decode(&exchange::read_import(&path)?)?;
            println!(
                "Imported {} subscriptions.",
                store.import_feeds(&candidates)?
            );
        }
        Command::ExportOpml { output } => {
            exchange::opml::export(store, &output)?
        }
        Command::Export { article, output } => {
            exchange::markdown::article(store, article, &output)?
        }
        Command::ExportNotebook { output } => {
            exchange::markdown::notebook(store, &output)?
        }
        Command::ExportNotes { directory } => {
            let report = exchange::incremental::notebook(store, &directory)?;
            println!(
                "{} written, {} unchanged, {} conflicts",
                report.written,
                report.unchanged,
                report.conflicts.len()
            );
            for conflict in report.conflicts {
                println!("Preserved: {}", conflict.display());
            }
        }
        _ => bail!("Command does not describe an import/export operation"),
    }
    Ok(())
}

fn sync_subscriptions(path: &Path) -> Result<()> {
    let (feeds, failures) = source::collect_once(path)?;
    println!("Collected {feeds} feeds; {failures} failures.");
    if failures > 0 {
        bail!(
            "Collection failed; run lume feeds --errors and lume servers to inspect errors"
        );
    }
    Ok(())
}

fn undo_change(store: &Store) -> Result<()> {
    let message = if store.undo()? {
        "Personal edit undone."
    } else {
        "Nothing to undo."
    };
    println!("{message}");
    Ok(())
}

fn search(store: &Store, text: String, limit: u16) -> Result<()> {
    let page = store.query_articles(&ArticleQuery {
        view: View::All,
        text,
        offset: 0,
        limit: usize::from(limit),
        retained: None,
    })?;
    for article in page.articles {
        println!(
            "{}\t{}\t{}\t{}",
            article.id, article.title, article.feed, article.url
        );
    }
    eprintln!("{} total matches.", page.total);
    Ok(())
}

fn real_library(demo: bool) -> Result<()> {
    if demo {
        bail!("Demo sources are fictional; remove --demo for subscriptions");
    }
    Ok(())
}

fn network_enabled(offline: bool) -> Result<()> {
    if offline {
        bail!("Network disabled by --offline");
    }
    Ok(())
}

fn install_reading(store: &Store) -> Result<()> {
    let report = lume::catalog::install(store)?;
    println!(
        "Reading selection: {} sources; {} added or re-enabled; {} topic searches created.",
        report.total_feeds, report.added_feeds, report.added_searches,
    );
    println!("Run lume sync or open the TUI to collect articles.");
    Ok(())
}

fn discover(url: &str, offline: bool) -> Result<()> {
    network_enabled(offline)?;
    for feed in source::discovery::discover(url)? {
        println!("{}\t{}", feed.title, feed.url);
    }
    Ok(())
}
