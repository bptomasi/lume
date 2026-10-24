//! Ordered, bounded progress/layout writes never replace or silently drop edits.

use std::{
    collections::{BTreeMap, VecDeque},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow};

use crate::{
    model::{ContentSource, LayoutHistory, Workspace},
    store::Store,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Progress {
    pub(super) article: i64,
    pub(super) source: ContentSource,
    pub(super) revision: i64,
    pub(super) anchor: usize,
}

const WRITE_QUEUE: usize = 64;

type PositionKey = (i64, &'static str, i64);

impl Progress {
    fn key(self) -> PositionKey {
        (self.article, self.source.as_str(), self.revision)
    }

    pub(super) fn save(self, store: &Store) -> Result<()> {
        if self.source == ContentSource::Full {
            store.save_document_progress(
                self.article,
                self.revision,
                self.anchor,
            )
        } else {
            store.save_progress(self.article, self.anchor)
        }
    }
}

/// The latest 512 distinct admitted positions outlive the 64-slot write queue.
#[derive(Default)]
pub(super) struct ProgressMemory {
    positions: BTreeMap<PositionKey, Progress>,
    order: VecDeque<PositionKey>,
}

impl ProgressMemory {
    pub(super) fn get(
        &self,
        article: &i64,
        source: ContentSource,
        revision: i64,
    ) -> Option<&Progress> {
        self.positions.get(&(*article, source.as_str(), revision))
    }

    pub(super) fn remember(&mut self, progress: Progress) {
        let key = progress.key();
        if let Some(index) = self.order.iter().position(|id| *id == key) {
            self.order.remove(index);
        }
        self.order.push_back(key);
        self.positions.insert(key, progress);
        if self.order.len() > 512
            && let Some(oldest) = self.order.pop_front()
        {
            self.positions.remove(&oldest);
        }
    }

    pub(super) fn clear(&mut self) {
        self.positions.clear();
        self.order.clear();
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum SaveKey {
    Progress(PositionKey),
    Layout,
}

enum Write {
    Progress(Progress),
    Layout(Box<(Workspace, LayoutHistory)>),
    Flush(mpsc::SyncSender<()>),
}

impl Write {
    fn save(&self, store: &Store) -> Result<()> {
        match self {
            Self::Progress(progress) => progress.save(store),
            Self::Layout(state) => store.save_layouts(&state.0, &state.1),
            Self::Flush(_) => Ok(()),
        }
    }

    fn key(&self) -> Option<SaveKey> {
        match self {
            Self::Progress(progress) => Some(SaveKey::Progress(progress.key())),
            Self::Layout(_) => Some(SaveKey::Layout),
            Self::Flush(_) => None,
        }
    }
}

pub(super) struct SessionWriter {
    requests: mpsc::SyncSender<Write>,
    errors: mpsc::Receiver<String>,
    pending: Arc<AtomicUsize>,
    failures: Arc<AtomicUsize>,
}

impl SessionWriter {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        let (requests, receiver) = mpsc::sync_channel(WRITE_QUEUE);
        let (errors, outcomes) = mpsc::sync_channel(WRITE_QUEUE);
        let pending = Arc::new(AtomicUsize::new(0));
        let active = pending.clone();
        let failures = Arc::new(AtomicUsize::new(0));
        let failed = failures.clone();
        std::thread::Builder::new()
            .name("lume-session".into())
            .spawn(move || run(store, receiver, errors, active, failed))?;
        Ok(Self {
            requests,
            errors: outcomes,
            pending,
            failures,
        })
    }

    fn submit(&self, write: Write) -> Result<()> {
        anyhow::ensure!(
            self.failures.load(Ordering::Acquire) < WRITE_QUEUE,
            "Session recovery backlog is full · :w retries outstanding saves"
        );
        self.pending.fetch_add(1, Ordering::AcqRel);
        if let Err(error) = self.requests.try_send(write) {
            self.pending.fetch_sub(1, Ordering::AcqRel);
            return Err(anyhow!(
                "Session write not admitted: {error}; retry the action"
            ));
        }
        Ok(())
    }

    pub(super) fn progress(&self, progress: Progress) -> Result<()> {
        self.submit(Write::Progress(progress))
    }

    pub(super) fn layout(
        &self,
        workspace: &Workspace,
        history: &LayoutHistory,
    ) -> Result<()> {
        self.submit(Write::Layout(Box::new((
            workspace.clone(),
            history.clone(),
        ))))
    }

    pub(super) fn poll_error(&self) -> Option<String> {
        self.errors.try_recv().ok()
    }

    pub(super) fn pending(&self) -> bool {
        self.pending.load(Ordering::Acquire) != 0
    }

    pub(super) fn failed(&self) -> bool {
        self.failures.load(Ordering::Acquire) != 0
    }

    pub(super) fn barrier(&self) -> Result<Option<mpsc::Receiver<()>>> {
        let (done, completion) = mpsc::sync_channel(1);
        match self.requests.try_send(Write::Flush(done)) {
            Ok(()) => Ok(Some(completion)),
            Err(mpsc::TrySendError::Full(_)) => Ok(None),
            Err(mpsc::TrySendError::Disconnected(_)) => {
                Err(anyhow!("Session writer stopped"))
            }
        }
    }

    pub(super) fn flush(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let (done, completion) = mpsc::sync_channel(1);
        let mut write = Write::Flush(done);
        loop {
            match self.requests.try_send(write) {
                Ok(()) => break,
                Err(mpsc::TrySendError::Full(returned)) => write = returned,
                Err(mpsc::TrySendError::Disconnected(_)) => {
                    return Err(anyhow!("Session writer stopped"));
                }
            }
            anyhow::ensure!(
                Instant::now() < deadline,
                "Session queue did not drain within five seconds"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        completion
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .context("Session save did not finish within five seconds")?;
        anyhow::ensure!(
            !self.failed(),
            "{} session writes failed; check the saved library before closing",
            self.failures.load(Ordering::Acquire)
        );
        Ok(())
    }
}

fn run(
    store: Store,
    receiver: mpsc::Receiver<Write>,
    errors: mpsc::SyncSender<String>,
    active: Arc<AtomicUsize>,
    failed: Arc<AtomicUsize>,
) {
    // Admission stops at WRITE_QUEUE unresolved keys. Only the
    // already queued writes can add more: at most twice that bound.
    let mut unresolved = BTreeMap::new();
    while let Ok(write) = receiver.recv() {
        if let Write::Flush(done) = write {
            let keys = unresolved.keys().copied().collect::<Vec<_>>();
            for key in keys {
                if let Some(write) = unresolved.remove(&key) {
                    save_pending(
                        &store,
                        write,
                        &mut unresolved,
                        &errors,
                        &failed,
                    );
                }
            }
            let _ = done.send(());
            continue;
        }
        save_pending(&store, write, &mut unresolved, &errors, &failed);
        active.fetch_sub(1, Ordering::AcqRel);
    }
}

fn save_pending(
    store: &Store,
    write: Write,
    unresolved: &mut BTreeMap<SaveKey, Write>,
    errors: &mpsc::SyncSender<String>,
    failed: &AtomicUsize,
) {
    let Some(key) = write.key() else {
        return;
    };
    match save_with_retry(store, &write) {
        Ok(()) => {
            unresolved.remove(&key);
            failed.store(unresolved.len(), Ordering::Release);
        }
        Err(error) => {
            let operation = match key {
                SaveKey::Progress(_) => "reading position",
                SaveKey::Layout => "workspace and layout history",
            };
            unresolved.insert(key, write);
            let _ = errors
                .try_send(format!("Could not save {operation}: {error:#}"));
            // Observing failure must also make its queued diagnostic visible.
            // Publishing the count first races with :save-status on the UI lane.
            failed.store(unresolved.len(), Ordering::Release);
        }
    }
}

// Only idempotent progress/layout saves are retried, never private mutations.
fn save_with_retry(store: &Store, write: &Write) -> Result<()> {
    let mut attempt = 0;
    loop {
        let result = write.save(store);
        let transient = result.as_ref().err().is_some_and(|error| {
            matches!(
                error
                    .downcast_ref::<rusqlite::Error>()
                    .and_then(rusqlite::Error::sqlite_error_code),
                Some(
                    rusqlite::ErrorCode::DatabaseBusy
                        | rusqlite::ErrorCode::DatabaseLocked
                )
            )
        });
        if !transient || attempt == 2 {
            return result;
        }
        std::thread::sleep(Duration::from_millis(25 << attempt));
        attempt += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_eviction_keeps_recent_pending_positions_and_refreshed_ids() {
        let mut saved = ProgressMemory::default();
        for article in 0..513 {
            saved.remember(Progress {
                article,
                source: ContentSource::Feed,
                revision: 0,
                anchor: article as usize,
            });
        }
        assert!(saved.get(&0, ContentSource::Feed, 0).is_none());
        assert_eq!(
            saved
                .get(&512, ContentSource::Feed, 0)
                .map(|progress| progress.anchor),
            Some(512)
        );
        saved.remember(Progress {
            article: 1,
            source: ContentSource::Feed,
            revision: 0,
            anchor: 900,
        });
        saved.remember(Progress {
            article: 513,
            source: ContentSource::Feed,
            revision: 0,
            anchor: 10,
        });
        assert_eq!(
            saved
                .get(&1, ContentSource::Feed, 0)
                .map(|progress| progress.anchor),
            Some(900)
        );
        assert!(saved.get(&2, ContentSource::Feed, 0).is_none());
        assert_eq!(saved.positions.len(), 512);
    }
    #[test]
    fn feed_and_full_revisions_keep_independent_unconfirmed_positions() {
        let mut saved = ProgressMemory::default();
        for (source, revision, anchor) in [
            (ContentSource::Feed, 0, 10),
            (ContentSource::Full, 1, 80),
            (ContentSource::Full, 2, 120),
        ] {
            saved.remember(Progress {
                article: 7,
                source,
                revision,
                anchor,
            });
        }
        assert_eq!(
            saved
                .get(&7, ContentSource::Feed, 0)
                .map(|item| item.anchor),
            Some(10)
        );
        assert_eq!(
            saved
                .get(&7, ContentSource::Full, 1)
                .map(|item| item.anchor),
            Some(80)
        );
        assert_eq!(
            saved
                .get(&7, ContentSource::Full, 2)
                .map(|item| item.anchor),
            Some(120)
        );
        assert!(saved.get(&7, ContentSource::Full, 3).is_none());
    }
}
