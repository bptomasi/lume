//! Explicit maintenance owns an ordered, single-admission background lane.
//!
//! Unlike replaceable search work, an admitted cleanup/checkpoint is never
//! superseded. Progress is lossy and bounded; final results are acknowledged.
//! Escape closes the report; :offline cancel stops work between requests.

mod commands;
mod operation;
mod report;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};

use anyhow::{Context, Result, ensure};

use crate::{
    model::{OfflineLimits, OfflineState, ReadingSession, View},
    storage::Cleanup,
};

use super::{App, Mode};

#[derive(Clone, Debug)]
pub(super) enum Request {
    Offline {
        view: View,
        limits: OfflineLimits,
    },
    Resume,
    Release,
    OfflineStatus,
    Storage(crate::storage::Unit),
    StorageDetails(crate::storage::Unit),
    Clean(Cleanup),
    Plan(u16),
    Start(ReadingSession),
    Session {
        advance: bool,
    },
    EndSession,
    Retention {
        action: RetentionAction,
        article: Option<i64>,
    },
}

#[derive(Clone, Copy, Debug)]
pub(super) enum RetentionAction {
    Explain(i64),
    Status,
    Preview,
    Apply,
    Automatic,
}

enum Outcome {
    Text(String),
    RetentionStatus(String),
    Offline(OfflineState),
    Plan(ReadingSession),
    Session(ReadingSession),
    Retention {
        report: crate::retention::Report,
        preview: bool,
    },
    AutomaticRetention {
        report: Option<crate::retention::Report>,
        delay: std::time::Duration,
    },
}

enum Update {
    Progress(String),
    Finished(Result<Outcome>),
}

pub(super) struct Maintenance {
    requests: mpsc::SyncSender<Request>,
    results: mpsc::Receiver<Update>,
    cancel: Arc<AtomicBool>,
    pending: bool,
}

impl Maintenance {
    fn start(path: PathBuf) -> Result<Self> {
        let (requests, receiver) = mpsc::sync_channel(1);
        let (sender, results) = mpsc::sync_channel(2);
        let cancel = Arc::new(AtomicBool::new(false));
        let stopped = cancel.clone();
        std::thread::Builder::new()
            .name("lume-maintenance".into())
            .spawn(move || {
                while let Ok(request) = receiver.recv() {
                    let result =
                        operation::perform(&path, request, &stopped, |text| {
                            let _ = sender.try_send(Update::Progress(text));
                        });
                    if sender.send(Update::Finished(result)).is_err() {
                        break;
                    }
                }
            })?;
        Ok(Self {
            requests,
            results,
            cancel,
            pending: false,
        })
    }
}

impl Drop for Maintenance {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}

impl App {
    pub(super) fn submit_maintenance(
        &mut self,
        request: Request,
    ) -> Result<()> {
        self.admit_maintenance(request)?;
        self.show_document(
            "Working · Esc closes report",
            "Working in the background…",
            None,
        );
        Ok(())
    }

    fn admit_maintenance(&mut self, request: Request) -> Result<()> {
        if self.maintenance.is_none() {
            self.maintenance =
                Some(Maintenance::start(self.store.database_path()?)?);
        }
        let worker = self
            .maintenance
            .as_mut()
            .context("Maintenance worker unavailable")?;
        ensure!(
            !worker.pending,
            "Maintenance is busy; :offline cancel requests cancellation"
        );
        worker.cancel.store(false, Ordering::Relaxed);
        worker
            .requests
            .try_send(request)
            .context("Maintenance request was not admitted")?;
        worker.pending = true;
        Ok(())
    }

    /// Apply bounded progress/completions without disk or network work on input.
    ///
    /// # Errors
    /// Returns an acknowledged background failure; committed batches are retained.
    pub fn poll_maintenance(&mut self) -> Result<bool> {
        if self.retention_due() {
            self.admit_maintenance(Request::Retention {
                action: RetentionAction::Automatic,
                article: self.current().map(|article| article.id),
            })?;
        }
        let Some(worker) = &mut self.maintenance else {
            return Ok(false);
        };
        let update = match worker.results.try_recv() {
            Ok(update) => update,
            Err(mpsc::TryRecvError::Empty) => return Ok(false),
            Err(mpsc::TryRecvError::Disconnected) => {
                if worker.pending {
                    worker.pending = false;
                    anyhow::bail!(
                        "Maintenance worker stopped before acknowledgement; inspect its checkpoint"
                    );
                }
                return Ok(false);
            }
        };
        if matches!(update, Update::Finished(_)) {
            worker.pending = false;
        }
        let changed = !matches!(
            &update,
            Update::Finished(Ok(Outcome::AutomaticRetention {
                report: None,
                ..
            }))
        );
        self.apply_maintenance_update(update)?;
        Ok(changed)
    }

    fn apply_maintenance_update(&mut self, update: Update) -> Result<()> {
        let visible = matches!(
            &self.mode,
            Mode::Document(view) if view.title == "Working · Esc closes report"
        );
        match update {
            Update::Progress(text) => {
                if visible {
                    self.show_document(
                        "Working · Esc closes report",
                        &text,
                        None,
                    );
                }
            }
            Update::Finished(result) => match result {
                Ok(outcome) => self.finish_maintenance(outcome, visible)?,
                Err(error) => {
                    if visible {
                        self.show_document(
                            "Background operation failed · Esc closes",
                            &format!(
                                r#"{error:#}

Committed batches remain intact.
Inspect the current checkpoint before retrying."#
                            ),
                            None,
                        );
                    }
                    return Err(error);
                }
            },
        }
        Ok(())
    }

    /// Whether explicit maintenance still awaits acknowledgement.
    pub fn maintenance_pending(&self) -> bool {
        self.maintenance
            .as_ref()
            .is_some_and(|worker| worker.pending)
    }

    fn finish_maintenance(
        &mut self,
        outcome: Outcome,
        visible: bool,
    ) -> Result<()> {
        match outcome {
            Outcome::AutomaticRetention { report, delay } => {
                self.schedule_retention(delay);
                if let Some(report) = report {
                    self.finish_retention(report, false, false)?;
                }
            }
            Outcome::Retention { report, preview } => {
                self.finish_retention(report, preview, visible)?;
            }
            Outcome::RetentionStatus(text) if visible => {
                self.show_document("Retention · Esc closes", &text, None)
            }
            Outcome::Text(text) if visible => {
                self.show_document("Storage · Esc closes", &text, None)
            }
            Outcome::Offline(state) if visible => {
                self.show_document(
                    "Offline package · Esc closes",
                    &report::offline(&state),
                    None,
                );
            }
            Outcome::Plan(session) if visible => {
                self.show_reading_plan(session);
            }
            Outcome::Session(session) => {
                self.show_reading_session(session, visible)?
            }
            _ => {
                self.status = concat!(
                    "Background operation completed · ",
                    ":offline status · :session status · :storage"
                )
                .into();
            }
        }
        Ok(())
    }

    fn show_reading_plan(&mut self, session: ReadingSession) {
        let confirm = (!session.items.is_empty())
            .then(|| super::ChoiceAction::StartSession(session.clone()));
        self.show_document(
            "Reading plan · Enter starts · Esc cancels",
            &report::session(&session),
            confirm,
        );
    }

    fn finish_retention(
        &mut self,
        report: crate::retention::Report,
        preview: bool,
        visible: bool,
    ) -> Result<()> {
        if !preview && (report.articles > 0 || report.full_texts > 0) {
            self.reload()?;
        }
        if visible {
            let confirm = (preview && !report.interrupted)
                .then_some(super::ChoiceAction::ApplyRetention);
            self.show_document(
                if preview {
                    "Retention preview · Enter applies · Esc closes"
                } else {
                    "Retention · Esc closes"
                },
                &report.text(preview),
                confirm,
            );
        } else if report.articles + report.full_texts + report.images > 0 {
            self.status = format!(
                "Retention: {} articles, {} Full caches, {} images",
                report.articles, report.full_texts, report.images
            );
        }
        Ok(())
    }

    fn show_reading_session(
        &mut self,
        session: ReadingSession,
        visible: bool,
    ) -> Result<()> {
        if !visible {
            self.status =
                "Reading session saved · :session status resumes it".into();
            return Ok(());
        }
        let confirm = session
            .items
            .get(session.position)
            .map(|item| super::ChoiceAction::JumpArticle(item.article));
        self.show_document(
            "Reading session · Enter resumes · :session next advances",
            &report::session(&session),
            confirm,
        );
        Ok(())
    }
}
