//! Durable session acknowledgement and progress deduplication.
//!
//! Failed admission never advances remembered progress. A worker-reported
//! failure invalidates deduplication so the next explicit save can retry it.
//! The session's background writer is installed once and keeps its queued work.

use super::session_writer::{Progress, ProgressMemory, SessionWriter};
use crate::{
    model::{ContentSource, LayoutHistory, Workspace},
    store::Store,
};
use anyhow::{Result, ensure};
use std::{cell::RefCell, path::Path, sync::mpsc::Receiver};

#[derive(Default)]
pub(super) struct SessionState {
    writer: Option<SessionWriter>,
    progress: RefCell<ProgressMemory>,
    problem: Option<String>,
}

impl SessionState {
    pub(super) fn enable(&mut self, path: &Path) -> Result<()> {
        ensure!(self.writer.is_none(), "Session writer is already enabled");
        self.writer = Some(SessionWriter::start(path)?);
        Ok(())
    }

    pub(super) fn pending(&self) -> bool {
        self.writer.as_ref().is_some_and(SessionWriter::pending)
    }

    pub(super) fn failed(&self) -> bool {
        self.writer.as_ref().is_some_and(SessionWriter::failed)
    }

    pub(super) fn barrier(&self) -> Result<Option<Receiver<()>>> {
        match &self.writer {
            Some(writer) => writer.barrier(),
            None => Ok(None),
        }
    }

    pub(super) fn flush(&self) -> Result<()> {
        if let Some(writer) = &self.writer {
            writer.flush()?;
        }
        Ok(())
    }

    pub(super) fn poll_problem(&mut self) -> Option<String> {
        let error = self.writer.as_ref()?.poll_error()?;
        self.problem = Some(error.clone());
        if self.failed() {
            self.progress.borrow_mut().clear();
        }
        Some(error)
    }

    pub(super) fn problem(&self) -> Option<&str> {
        self.problem.as_deref()
    }

    pub(super) fn anchor(
        &self,
        article: i64,
        source: ContentSource,
        revision: i64,
    ) -> Option<usize> {
        self.progress
            .borrow()
            .get(&article, source, revision)
            .map(|progress| progress.anchor)
    }

    pub(super) fn save_progress(
        &self,
        store: &Store,
        progress: Progress,
    ) -> Result<()> {
        if self.progress.borrow().get(
            &progress.article,
            progress.source,
            progress.revision,
        ) == Some(&progress)
        {
            return Ok(());
        }
        if let Some(writer) = &self.writer {
            writer.progress(progress)?;
        } else {
            progress.save(store)?;
        }
        self.progress.borrow_mut().remember(progress);
        Ok(())
    }

    pub(super) fn save_layout(
        &self,
        store: &Store,
        workspace: &Workspace,
        history: &LayoutHistory,
    ) -> Result<()> {
        if let Some(writer) = &self.writer {
            writer.layout(workspace, history)
        } else {
            store.save_layouts(workspace, history)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enabling_twice_preserves_the_existing_writer_and_its_saves() -> Result<()>
    {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("library.sqlite3");
        let store = Store::open(&path)?;
        let mut session = SessionState::default();
        session.enable(&path)?;
        let workspace = Workspace {
            fullscreen_width: 94,
            ..Workspace::default()
        };
        session.save_layout(
            &store,
            &workspace,
            &LayoutHistory::new(&workspace),
        )?;

        assert!(session.enable(&path).is_err());
        session.flush()?;
        assert_eq!(Store::open(&path)?.workspace()?.fullscreen_width, 94);
        assert!(!session.pending());
        Ok(())
    }
}
