//! Notebook origins and suspended editors have one explicit lifecycle.

use super::{Focus, Notebook, NotebookState, Origin, Workspace};
use anyhow::Result;

impl Origin {
    pub(super) fn capture(
        workspace: &Workspace,
        layouts: &crate::model::LayoutHistory,
        focus: Focus,
    ) -> Self {
        Self {
            layout: crate::model::PanelLayout::capture(workspace),
            layouts: layouts.clone(),
            focus,
        }
    }
}

impl NotebookState {
    pub(in crate::app) fn suspended_editor(
        &self,
    ) -> Option<&crate::app::Input> {
        self.editor.as_deref()
    }

    pub(in crate::app) fn install_recent(&mut self, recent: Vec<String>) {
        if let Some(book) = &mut self.active {
            book.install_recent(recent);
        }
    }

    pub(super) fn open(
        &mut self,
        path: &std::path::Path,
        query: &str,
        article: Option<i64>,
        origin: Origin,
    ) -> Result<bool> {
        crate::search::notes::validate(query)?;
        if let Some(book) = &mut self.active {
            book.query = query.into();
            book.article = article;
            book.offset = 0;
            book.refresh()?;
            return Ok(false);
        }
        let mut book = Notebook::start(path, query, article, origin)?;
        book.refresh()?;
        self.returning = None;
        self.active = Some(Box::new(book));
        Ok(true)
    }

    pub(super) fn close(&mut self) -> Option<Origin> {
        self.active.take().map(|book| book.origin)
    }

    pub(super) fn suspend_source(&mut self) -> Option<Origin> {
        let book = self.active.take()?;
        let origin = book.origin.clone();
        self.returning = Some(book);
        Some(origin)
    }

    pub(super) fn resume_source(&mut self, origin: Origin) -> Result<bool> {
        let Some(book) = &mut self.returning else {
            return Ok(false);
        };
        // Keep the suspended workspace available if the query is rejected.
        book.refresh()?;
        book.origin = origin;
        self.active = self.returning.take();
        Ok(true)
    }

    pub(super) fn invalidate(&mut self) {
        if let Some(book) = &mut self.active
            && let Err(error) = book.refresh()
        {
            book.error = error.to_string();
        }
    }

    pub(super) fn pending(&self) -> bool {
        self.active
            .as_ref()
            .is_some_and(|book| book.loading || book.context_loading)
    }

    pub(super) fn poll(&mut self) -> Result<bool> {
        let Some(book) = &mut self.active else {
            return Ok(false);
        };
        book.poll()
    }

    pub(super) fn suspend_editor(&mut self, input: Box<crate::app::Input>) {
        self.editor = Some(input);
    }

    pub(super) fn resume_editor(&mut self) -> Option<Box<crate::app::Input>> {
        self.editor.take()
    }

    pub(super) fn layout_snapshot(
        &mut self,
        workspace: &Workspace,
        layouts: &crate::model::LayoutHistory,
    ) -> Option<(Workspace, &crate::model::LayoutHistory)> {
        let book = self.active.as_mut()?;
        book.origin.layouts.profiles.clone_from(&layouts.profiles);
        let mut saved = workspace.clone();
        saved.notebook_layout =
            Some(crate::model::PanelLayout::capture(workspace));
        book.origin.layout.apply(&mut saved);
        Some((saved, &book.origin.layouts))
    }
}
