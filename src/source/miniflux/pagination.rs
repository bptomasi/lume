//! Identity pagination remains stable when newer entries arrive or older ones vanish.
use super::{EntryPage, RemoteEntry, ServerFeed};
use crate::model::ServerHistory;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    sync::atomic::{AtomicBool, Ordering},
};

pub(super) struct Collected {
    pub(super) entries: BTreeMap<i64, RemoteEntry>,
    pub(super) after_id: i64,
    pub(super) complete: bool,
    pub(super) history: ServerHistory,
}

struct Cursor {
    id: i64,
    history: ServerHistory,
    entries: BTreeMap<i64, RemoteEntry>,
}

pub(super) fn collect_pages(
    server: &ServerFeed,
    stopped: &AtomicBool,
    mut fetch: impl FnMut(i64, usize) -> Result<EntryPage>,
) -> Result<Collected> {
    server.sync.validate()?;
    let mut cursor = Cursor {
        id: if server.initialized {
            server.sync_after_id
        } else {
            server.history.before_id
        },
        history: server.history.clone(),
        entries: BTreeMap::new(),
    };
    for _ in 0..server.sync.pages_per_sync {
        ensure!(!stopped.load(Ordering::Relaxed), "Collection cancelled");
        let limit = page_limit(server, cursor.history.loaded);
        if limit == 0 {
            return Ok(cursor.finish(server, true));
        }
        let page = fetch(cursor.id, limit)?;
        if cursor.accept(server, page, limit)? {
            return Ok(cursor.finish(server, true));
        }
    }
    Ok(cursor.finish(server, false))
}

fn page_limit(server: &ServerFeed, loaded: u64) -> usize {
    let limit = usize::from(server.sync.entries_per_page);
    if server.initialized || server.sync.initial_entries == 0 {
        limit
    } else {
        limit.min(
            u64::from(server.sync.initial_entries).saturating_sub(loaded)
                as usize,
        )
    }
}

impl Cursor {
    fn accept(
        &mut self,
        server: &ServerFeed,
        page: EntryPage,
        limit: usize,
    ) -> Result<bool> {
        ensure!(page.entries.len() <= limit, "Oversized entry page");
        ensure!(
            page.total <= i64::MAX as usize,
            "Invalid remote history total"
        );
        let count = page.entries.len();
        if !server.initialized {
            self.history.total = self
                .history
                .total
                .max(self.history.loaded.saturating_add(page.total as u64));
            self.history.loaded = self
                .history
                .loaded
                .checked_add(count as u64)
                .context("Remote history count overflow")?;
        }
        for entry in page.entries {
            ensure!(entry.id > 0, "Remote entry needs a positive ID");
            ensure!(
                if server.initialized {
                    entry.id > self.id
                } else {
                    self.id == 0 || entry.id < self.id
                },
                "Remote entry IDs are not strictly ordered"
            );
            self.id = entry.id;
            self.entries.insert(entry.id, entry);
        }
        if !server.initialized {
            self.history.before_id = self.id;
        }
        Ok(count < limit
            || page.total <= count
            || page_limit(server, self.history.loaded) == 0)
    }

    fn finish(self, server: &ServerFeed, complete: bool) -> Collected {
        Collected {
            entries: self.entries,
            after_id: if server.initialized && !complete {
                self.id
            } else {
                0
            },
            complete,
            history: self.history,
        }
    }
}
