//! Ordered personal writes own one connection and a bounded admission/result queue.
//!
//! An admitted mutation is never replaced. Results retain captured identities;
//! later navigation cannot redirect them. Explicit shutdown drains writes.
//! Dropping disconnects the adapter; the worker finishes admitted transactions.

mod library;
pub(super) mod operation;
pub(super) use library::LibraryEdit;
mod review;
pub(super) use review::review;

use std::{cell::RefCell, collections::VecDeque, path::Path, sync::mpsc};

use anyhow::{Result, anyhow};

use crate::model::{Comment, DraftTarget, NoteChange, NoteDraft, UndoEffect};

#[derive(Clone)]
pub(super) enum Task {
    SavePage {
        document: std::sync::Arc<crate::model::WebDocument>,
        visit: u64,
    },
    Write(NoteDraft),
    Query(QueryEdit),
    Edit(Edit),
    Articles {
        articles: std::collections::BTreeSet<i64>,
        edit: crate::model::BulkEdit,
    },
    Library(LibraryEdit),
    Undo,
    Export(std::path::PathBuf),
    ExportArticles(std::path::PathBuf),
    LinkArticle {
        article: i64,
        note: i64,
        related: i64,
    },
    Checkpoint(NoteDraft),
    Discard(DraftTarget),
}

#[derive(Clone)]
pub(super) enum QueryEdit {
    Save { name: String, query: String },
    Delete(String),
    Remember(String),
}

#[derive(Clone)]
pub(super) enum Edit {
    Delete {
        article: i64,
        note: i64,
    },
    Review {
        target: super::ReviewTarget,
        days: u16,
        postpone: bool,
    },
    Metadata {
        article: i64,
        note: i64,
        change: NoteChange,
    },
    Tags {
        article: i64,
        note: i64,
        tags: Vec<String>,
    },
    RestoreVersion {
        current: std::sync::Arc<crate::model::AnnotationSnapshot>,
        version: i64,
    },
    Reanchor {
        current: std::sync::Arc<crate::model::AnnotationSnapshot>,
        passage: crate::model::Passage,
    },
}

pub(super) enum Outcome {
    SavedPage {
        article: i64,
        visit: u64,
    },
    Written {
        draft: NoteDraft,
        target: DraftTarget,
        comments: Result<Vec<Comment>>,
    },
    Changed {
        effect: UndoEffect,
        comments: Result<Vec<Comment>>,
    },
    Articles,
    Checkpoint,
    Collections,
    SearchHistory(Vec<String>),
    Exported(crate::exchange::incremental::ExportReport),
}

struct Job {
    ticket: u64,
    task: Task,
    started: Option<std::time::Instant>,
}

pub(super) struct Completion {
    pub(super) ticket: u64,
    pub(super) result: Result<Outcome>,
    pub(super) started: Option<std::time::Instant>,
}

pub(super) struct PersonalWrites {
    requests: mpsc::SyncSender<Job>,
    results: mpsc::Receiver<Completion>,
    next: u64,
    pending: RefCell<VecDeque<u64>>,
    diagnostics: bool,
}

impl PersonalWrites {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let store = crate::store::Store::open(path)?;
        let (requests, receiver) = mpsc::sync_channel::<Job>(16);
        let (sender, results) = mpsc::sync_channel(16);
        let pending = RefCell::new(VecDeque::new());
        std::thread::Builder::new()
            .name("lume-personal-writes".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let result = operation::retry(&store, job.task);
                    let _ = sender.send(Completion {
                        ticket: job.ticket,
                        result,
                        started: job.started,
                    });
                }
            })?;
        Ok(Self {
            requests,
            results,
            next: 0,
            pending,
            diagnostics: false,
        })
    }

    pub(super) fn submit(&mut self, task: Task) -> Result<u64> {
        // Bounded request/result queues prevent live ticket aliasing on wrap.
        self.next = self.next.wrapping_add(1);
        if let Err(error) = self.requests.try_send(Job {
            ticket: self.next,
            task,
            started: self.diagnostics.then(std::time::Instant::now),
        }) {
            return Err(anyhow!(
                "Personal write not admitted: {error}; retry the action"
            ));
        }
        self.pending.borrow_mut().push_back(self.next);
        Ok(self.next)
    }

    pub(super) fn poll(&self) -> Option<Completion> {
        let completion = match self.results.try_recv() {
            Ok(completion) => completion,
            Err(mpsc::TryRecvError::Empty) => return None,
            Err(mpsc::TryRecvError::Disconnected) => Completion {
                ticket: *self.pending.borrow().front()?,
                result: Err(anyhow!(
                    "Personal writer stopped before acknowledgement; inspect the library before retrying"
                )),
                started: None,
            },
        };
        self.pending.borrow_mut().pop_front();
        Some(completion)
    }

    pub(super) fn pending(&self) -> bool {
        !self.pending.borrow().is_empty()
    }

    pub(super) fn set_diagnostics(&mut self, enabled: bool) {
        self.diagnostics = enabled;
    }
}

pub(super) struct PendingWrite {
    pub(super) ticket: u64,
    pub(super) destination: super::draft_commands::Destination,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stopped_worker_acknowledges_each_pending_ticket_as_a_failure()
    -> Result<()> {
        let (requests, _receiver) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(1);
        let writer = PersonalWrites {
            requests,
            results,
            next: 2,
            pending: RefCell::new([1, 2].into()),
            diagnostics: false,
        };
        drop(sender);
        for ticket in [1, 2] {
            let result = writer
                .poll()
                .ok_or_else(|| anyhow!("Missing failure acknowledgement"))?;
            assert_eq!(result.ticket, ticket);
            assert!(result.result.is_err());
        }
        assert!(!writer.pending());
        assert!(writer.poll().is_none());
        Ok(())
    }
}
