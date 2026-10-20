//! Ordered personal writes own one connection and a bounded admission/result queue.
//!
//! An admitted mutation is never replaced. Results retain captured identities;
//! later navigation cannot redirect them. Explicit shutdown drains writes.
//! Dropping disconnects the adapter; the worker finishes admitted transactions.

mod operation;
mod review;
pub(super) use review::review;

use std::{cell::Cell, path::Path, sync::mpsc};

use anyhow::{Result, anyhow};

use crate::model::{Comment, DraftTarget, NoteChange, NoteDraft, UndoEffect};

#[derive(Clone)]
pub(super) enum Task {
    Write(NoteDraft),
    Query(QueryEdit),
    Edit(Edit),
    Read {
        articles: std::collections::BTreeSet<i64>,
        unread: bool,
    },
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
    Written {
        draft: NoteDraft,
        target: DraftTarget,
        comments: Result<Vec<Comment>>,
    },
    Changed {
        effect: UndoEffect,
        comments: Result<Vec<Comment>>,
    },
    Read,
    Checkpoint,
    Collections,
    SearchHistory(Vec<String>),
    Exported(crate::exchange::incremental::ExportReport),
}

struct Job {
    ticket: u64,
    task: Task,
}

pub(super) struct Completion {
    pub(super) ticket: u64,
    pub(super) result: Result<Outcome>,
}

pub(super) struct PersonalWrites {
    requests: mpsc::SyncSender<Job>,
    results: mpsc::Receiver<Completion>,
    next: u64,
    pending: Cell<usize>,
}

impl PersonalWrites {
    pub(super) fn start(path: &Path) -> Result<Self> {
        let store = crate::store::Store::open(path)?;
        let (requests, receiver) = mpsc::sync_channel::<Job>(16);
        let (sender, results) = mpsc::sync_channel(16);
        let pending = Cell::new(0);
        std::thread::Builder::new()
            .name("lume-personal-writes".into())
            .spawn(move || {
                while let Ok(job) = receiver.recv() {
                    let result = operation::retry(&store, job.task);
                    let _ = sender.send(Completion {
                        ticket: job.ticket,
                        result,
                    });
                }
            })?;
        Ok(Self {
            requests,
            results,
            next: 0,
            pending,
        })
    }

    pub(super) fn submit(&mut self, task: Task) -> Result<u64> {
        // Bounded request/result queues prevent live ticket aliasing on wrap.
        self.next = self.next.wrapping_add(1);
        self.pending.set(self.pending.get() + 1);
        if let Err(error) = self.requests.try_send(Job {
            ticket: self.next,
            task,
        }) {
            self.pending.set(self.pending.get() - 1);
            return Err(anyhow!(
                "Personal write not admitted: {error}; retry the action"
            ));
        }
        Ok(self.next)
    }

    pub(super) fn poll(&self) -> Option<Completion> {
        let result = self.results.try_recv().ok()?;
        // A durable mutation remains pending until its UI acknowledgement,
        // including failures. Shutdown must also drain completed outcomes.
        self.pending.set(self.pending.get() - 1);
        Some(result)
    }

    pub(super) fn pending(&self) -> bool {
        self.pending.get() != 0
    }
}

pub(super) struct PendingWrite {
    pub(super) ticket: u64,
    pub(super) destination: super::draft_commands::Destination,
}
