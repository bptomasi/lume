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

enum Write {
    Progress(Progress),
    Layout(Box<(Workspace, LayoutHistory)>),
    Flush(mpsc::SyncSender<()>),
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
        let (requests, receiver) = mpsc::sync_channel(64);
        let (errors, outcomes) = mpsc::sync_channel(64);
        let pending = Arc::new(AtomicUsize::new(0));
        let active = pending.clone();
        let failures = Arc::new(AtomicUsize::new(0));
        let failed = failures.clone();
        std::thread::Builder::new()
            .name("lume-session".into())
            .spawn(move || {
                while let Ok(write) = receiver.recv() {
                    let result = match write {
                        Write::Progress(progress) => progress.save(&store),
                        Write::Layout(state) => {
                            store.save_layouts(&state.0, &state.1)
                        }
                        Write::Flush(done) => {
                            let _ = done.send(());
                            continue;
                        }
                    };
                    if let Err(error) = result {
                        failed.fetch_add(1, Ordering::AcqRel);
                        let _ = errors.try_send(format!("{error:#}"));
                    }
                    active.fetch_sub(1, Ordering::AcqRel);
                }
            })?;
        Ok(Self {
            requests,
            errors: outcomes,
            pending,
            failures,
        })
    }

    fn submit(&self, write: Write) -> Result<()> {
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
