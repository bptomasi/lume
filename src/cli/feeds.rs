//! Subscription display order is independent of durable local identities.

use std::io::{self, Write};

use anyhow::Result;
use clap::ValueEnum;
use lume::{content, store::Store};
use unicode_width::UnicodeWidthStr;

#[derive(Clone, Copy, Default, ValueEnum)]
pub(crate) enum Sort {
    #[default]
    Title,
    Id,
}

pub(super) fn print(store: &Store, sort: Sort, errors: bool) -> Result<()> {
    let mut feeds = store.feeds()?;
    if errors {
        feeds.retain(|feed| !feed.error.is_empty());
    }
    if matches!(sort, Sort::Id) {
        feeds.sort_by_key(|feed| feed.id);
    }
    let id_width = feeds
        .iter()
        .map(|feed| feed.id.to_string().len())
        .max()
        .unwrap_or(0)
        .max(2);
    let rows = feeds
        .iter()
        .map(|feed| (feed, content::label(&feed.title, 200)))
        .collect::<Vec<_>>();
    let width = rows
        .iter()
        .map(|(_, title)| title.width())
        .max()
        .unwrap_or(0)
        .max(5);
    let mut output = io::stdout().lock();
    writeln!(output, "{:>id_width$}  {:width$}  URL", "ID", "TITLE")?;
    for (feed, title) in rows {
        let padding = " ".repeat(width.saturating_sub(title.width()));
        writeln!(
            output,
            "{:>id_width$}  {title}{padding}  {}",
            feed.id, feed.url
        )?;
        if !feed.error.is_empty() {
            let error = content::label(&feed.error, 1000);
            writeln!(output, "{:id_width$}  Last error: {error}", "")?;
        }
    }
    Ok(())
}
