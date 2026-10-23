//! Publisher revisions dirty a durable queue; maintenance parses outside write locks.
//! Generations prevent concurrent publisher updates from publishing stale URLs.
use super::Store;
use anyhow::{Result, ensure};
use pulldown_cmark::{Event, Parser, Tag};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    sync::atomic::{AtomicBool, Ordering},
};

struct Pending {
    article: i64,
    generation: i64,
    sources: [(String, String); 2],
}
struct Indexed {
    article: i64,
    generation: i64,
    urls: Vec<(i64, String)>,
}

impl Store {
    pub(crate) fn refresh_image_references(
        &self,
        cancel: &AtomicBool,
    ) -> Result<bool> {
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            let batch = self.image_reference_batch()?;
            if batch.is_empty() {
                return Ok(true);
            }
            let mut indexed = Vec::with_capacity(batch.len());
            for entry in batch {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(false);
                }
                indexed.push(index(entry, cancel)?);
            }
            if cancel.load(Ordering::Relaxed) {
                return Ok(false);
            }
            self.publish_image_references(indexed)?;
            std::thread::yield_now();
        }
    }

    fn image_reference_batch(&self) -> Result<Vec<Pending>> {
        Ok(self
            .connection
            .prepare_cached(include_str!(
                "sql/image_references/select_dirty.sql"
            ))?
            .query_map([], |row| {
                Ok(Pending {
                    article: row.get(0)?,
                    generation: row.get(1)?,
                    sources: [
                        (row.get(2)?, row.get(3)?),
                        (
                            row.get(4)?,
                            row.get::<_, Option<String>>(5)?
                                .unwrap_or_default(),
                        ),
                    ],
                })
            })?
            .collect::<rusqlite::Result<_>>()?)
    }

    fn publish_image_references(&self, indexed: Vec<Indexed>) -> Result<()> {
        let transaction = self.write_transaction()?;
        for entry in indexed {
            if transaction.execute(
                include_str!("sql/image_references/dequeue.sql"),
                (entry.article, entry.generation),
            )? == 0
            {
                continue;
            }
            transaction.execute(
                include_str!("sql/image_references/delete.sql"),
                [entry.article],
            )?;
            let mut insert = transaction.prepare_cached(include_str!(
                "sql/image_references/insert.sql"
            ))?;
            for (source, url) in entry.urls {
                let bucket = Sha256::digest(url.as_bytes())
                    .iter()
                    .take(8)
                    .map(|byte| format!("{byte:02x}"))
                    .collect::<String>();
                insert.execute((entry.article, source, url, bucket))?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub(crate) fn visit_image_urls(
        &self,
        cancel: &AtomicBool,
        mut visit: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let mut statement = self.connection.prepare_cached(include_str!(
            "sql/image_references/select_urls.sql"
        ))?;
        let mut rows = statement.query([])?;
        while !cancel.load(Ordering::Relaxed) {
            let Some(row) = rows.next()? else {
                break;
            };
            visit(&row.get::<_, String>(0)?)?;
        }
        Ok(())
    }
}

fn index(entry: Pending, cancel: &AtomicBool) -> Result<Indexed> {
    let mut urls = Vec::new();
    for (source, (base, body)) in entry.sources.into_iter().enumerate() {
        let mut unique = BTreeSet::new();
        let base = crate::source::feed_url(&base).ok();
        for event in Parser::new(&body) {
            if cancel.load(Ordering::Relaxed) {
                break;
            }
            let Event::Start(Tag::Image { dest_url, .. }) = event else {
                continue;
            };
            let url = crate::source::feed_url(&dest_url).ok().or_else(|| {
                crate::source::feed_url(
                    base.as_ref()?.join(&dest_url).ok()?.as_str(),
                )
                .ok()
            });
            if let Some(url) = url {
                unique.insert(url.to_string());
            }
            ensure!(
                unique.len() <= 100_000,
                "Article exceeds image reference limit"
            );
        }
        urls.extend(unique.into_iter().map(|url| (source as i64, url)));
    }
    Ok(Indexed {
        article: entry.article,
        generation: entry.generation,
        urls,
    })
}

#[cfg(test)]
mod tests;
