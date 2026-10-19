use super::Edit;
use crate::{
    model::{ArticlePage, ArticleQuery, RecommendationUpdate},
    store::Store,
};
use anyhow::{Result, anyhow};
use std::{path::Path, sync::mpsc};

struct Job {
    edit: Option<Edit>,
    query: Option<ArticleQuery>,
}

pub(super) struct Completed {
    pub(super) update: RecommendationUpdate,
    pub(super) page: Option<(ArticleQuery, ArticlePage)>,
}

pub(super) struct Worker {
    requests: mpsc::SyncSender<Job>,
    results: mpsc::Receiver<Result<Completed>>,
    pending: usize,
}

impl Worker {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let store = Store::open(path)?;
        let (requests, receiver) = mpsc::sync_channel::<Job>(8);
        let (sender, results) = mpsc::sync_channel(8);
        std::thread::Builder::new()
            .name("lume-recommendations".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let result = execute(&store, job);
                    let _ = sender.send(result);
                }
            })?;
        Ok(Self {
            requests,
            results,
            pending: 0,
        })
    }

    pub(super) fn submit(
        &mut self,
        edit: Option<Edit>,
        query: Option<ArticleQuery>,
    ) -> Result<()> {
        anyhow::ensure!(
            self.pending < 8,
            "Recommendation queue is full; wait for pending updates"
        );
        self.requests
            .try_send(Job { edit, query })
            .map_err(|error| {
                anyhow!("Recommendation request not admitted: {error}; retry")
            })?;
        self.pending += 1;
        Ok(())
    }

    pub(super) fn poll(&mut self) -> Option<Result<Completed>> {
        match self.results.try_recv() {
            Ok(result) => {
                self.pending = self.pending.saturating_sub(1);
                Some(result)
            }
            Err(mpsc::TryRecvError::Disconnected) if self.pending > 0 => {
                self.pending = 0;
                Some(Err(anyhow!("Recommendation worker disconnected")))
            }
            _ => None,
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.pending > 0
    }
}

fn execute(store: &Store, job: Job) -> Result<Completed> {
    if let Some(edit) = job.edit {
        retry(|| super::apply_edit(store, edit.clone()))?;
    }
    // A post-commit read failure never retries the admitted preference edit.
    let update = retry(|| store.refresh_recommendations())?;
    let page = job
        .query
        .map(|query| {
            let page = store.query_articles(&query)?;
            Ok::<_, anyhow::Error>((query, page))
        })
        .transpose()?;
    Ok(Completed { update, page })
}

fn retry<T>(mut operation: impl FnMut() -> Result<T>) -> Result<T> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match operation() {
            Err(error)
                if busy(&error) && std::time::Instant::now() < deadline =>
            {
                std::thread::sleep(std::time::Duration::from_millis(8));
            }
            result => return result,
        }
    }
}

fn busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| matches!(cause.downcast_ref::<rusqlite::Error>(),
        Some(rusqlite::Error::SqliteFailure(error, _)) if matches!(error.code,
            rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked)))
}
