//! Reader cache, quote capture and durable reading position.

use anyhow::Result;

use super::{App, View};
use crate::{command::Command, content::BodyKind};

/// Identity of the actual displayed publisher text, independent of selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReaderIdentity {
    /// Durable article whose bytes produced the reader projection.
    pub article: i64,
    /// Feed or independently cached Full source.
    pub source: crate::model::ContentSource,
    /// Full revision, or zero for Feed.
    pub revision: i64,
}

impl App {
    pub(super) fn guard_stale_command(&self, command: Command) -> Result<()> {
        if self.notebook().is_some() || !self.reader_stale() {
            return Ok(());
        }
        anyhow::ensure!(
            matches!(
                command,
                Command::PanelFocus
                    | Command::PanelClose
                    | Command::PanelMove
                    | Command::PanelResize
                    | Command::PanelDock
                    | Command::PanelSplit
                    | Command::Help
                    | Command::Workspace
                    | Command::Fullscreen
                    | Command::Zen
                    | Command::Sidebar
                    | Command::Collapse
                    | Command::Reader
                    | Command::Annotations
                    | Command::LayoutUndo
                    | Command::LayoutRedo
                    | Command::FullscreenWidth
                    | Command::Justify
                    | Command::Refresh
                    | Command::Read
                    | Command::Favorites
                    | Command::Continue
                    | Command::SinceLastVisit
                    | Command::Undo
            ),
            "Wait for the selected article; the previous source is still displayed"
        );
        Ok(())
    }

    /// Apply ready syntax styles without changing text, cursor or selection.
    pub fn poll_reader(&mut self) -> bool {
        if !self.reader.poll_render_worker() {
            return false;
        }
        if self.nav.navigation_loading() || self.nav.projecting_navigation() {
            return false;
        }
        if self.reader.render_worker_stopped() {
            self.status =
                "Syntax worker stopped · plain code remains readable".into();
        }
        self.reader.reproject_ready(self.render_context());
        true
    }

    /// Whether the selected publisher source has not produced readable rows yet.
    pub fn article_preparing(&self) -> bool {
        self.nav.navigation_loading()
            || (*self.nav.view() != View::Annotated
                && self.reader.view().body_kind == BodyKind::Text
                && !self.reader.source_ready())
    }

    /// The previous article stays visible while a new selection is loading.
    /// Source-bound actions must wait for the matching content identity.
    pub fn reader_stale(&self) -> bool {
        self.reading_article().is_some()
            && (self.nav.navigation_loading()
                || self.nav.projecting_navigation()
                || self.current().map(|article| article.id)
                    != self.reading_article())
    }

    /// Metadata for the source actually displayed, including a retained preview.
    pub fn displayed_article(&self) -> Option<&crate::model::Article> {
        if self.reader_stale() {
            self.reader.view().displayed_article
        } else {
            self.current()
        }
    }

    pub(super) fn load_current_deferred(&mut self) -> Result<()> {
        self.cancel_reader_pointer();
        self.vim.clear_input();
        let Some(id) = self.current().map(|article| article.id) else {
            self.clear_current();
            return Ok(());
        };
        let feed = self.current().map_or(0, |article| article.feed_id);
        if self.navigation_worker.is_some() {
            self.defer_navigation(super::navigation_worker::Request::Article {
                id,
                feed,
            });
            return Ok(());
        }
        self.load_current()
    }

    pub(super) fn load_current(&mut self) -> Result<()> {
        if let Some(worker) = &mut self.navigation_worker {
            worker.cancel();
        }
        self.nav.complete_navigation();
        self.cancel_reader_pointer();
        let Some(article) = self.current() else {
            self.clear_current();
            return Ok(());
        };
        let data = super::reading_data::ReadingData::load(
            &self.store,
            article.id,
            article.feed_id,
        )?;
        self.immediate_layout = true;
        let result = self.install_reading_data(data);
        self.immediate_layout = false;
        result
    }

    pub(super) fn install_reading_data(
        &mut self,
        mut data: super::reading_data::ReadingData,
    ) -> Result<()> {
        anyhow::ensure!(
            self.current()
                .is_some_and(|article| article.id == data.article),
            "Reader result belongs to an obsolete article selection"
        );
        let article = self.current().cloned().ok_or_else(|| {
            anyhow::anyhow!("Reader result has no selected article")
        })?;
        let progress = self.remembered_progress(&data);
        self.reader.open_document(
            article,
            &mut data,
            self.workspace.content_source,
            self.render_context(),
            progress,
        )?;
        Ok(())
    }

    fn remembered_progress(
        &self,
        data: &super::reading_data::ReadingData,
    ) -> super::reader_state::RememberedProgress {
        use crate::model::ContentSource;
        let memory = self.progress_memory.borrow();
        super::reader_state::RememberedProgress {
            feed: memory
                .get(&data.article, ContentSource::Feed, 0)
                .map(|progress| progress.anchor),
            full: data.document.as_ref().and_then(|document| {
                memory
                    .get(&data.article, ContentSource::Full, document.revision)
                    .map(|progress| progress.anchor)
            }),
        }
    }

    pub(super) fn save_reading_position(&self) -> Result<()> {
        let Some(article) = self
            .current()
            .filter(|article| Some(article.id) == self.reading_article())
        else {
            return Ok(());
        };
        let Some((identity, anchor)) = self.reader.reading_position() else {
            return Ok(());
        };
        let progress = super::session_writer::Progress {
            article: article.id,
            source: identity.source,
            revision: identity.revision,
            anchor,
        };
        if self.progress_memory.borrow().get(
            &article.id,
            self.reader.source(),
            self.reader.revision(),
        ) == Some(&progress)
        {
            return Ok(());
        }
        if let Some(writer) = &self.session_writer {
            writer.progress(progress)?;
        } else {
            progress.save(&self.store)?;
        }
        self.progress_memory.borrow_mut().remember(progress);
        Ok(())
    }

    /// Save the layout, active destination and reading position before exit.
    ///
    /// # Errors
    /// Returns storage failures; the terminal runner still restores its state.
    pub fn save_session(&mut self) -> Result<()> {
        let position = self.save_reading_position();
        self.workspace.article = self.session_article();
        self.workspace.view = self.nav.view().clone();
        self.workspace.page_offset = self.nav.page_offset();
        let preferences = self.persist_preferences();
        finish_writes([position, preferences])
    }

    pub(super) fn persist_preferences(&mut self) -> Result<()> {
        self.remember_appearance_write();
        if self.persist_notebook_layout()? {
            return Ok(());
        }
        self.layouts
            .observe(crate::model::PanelLayout::capture(&self.workspace));
        if let Some(writer) = &self.session_writer {
            return writer.layout(&self.workspace, &self.layouts);
        }
        self.store.save_layouts(&self.workspace, &self.layouts)
    }

    /// Persist the session boundary only when leaving the terminal application.
    ///
    /// # Errors
    /// Returns local storage errors while preserving terminal cleanup.
    pub fn end_session(&mut self) -> Result<()> {
        // Independent durability boundaries must all run, even after a failed
        // background edit. Disposable ranking must never suppress note recovery.
        let recommendations = self.flush_recommendations();
        let draft = self.checkpoint_draft();
        // Global Ctrl-c can leave this modal without its explicit confirmation.
        if let super::Mode::Settings(draft) = &self.mode {
            self.workspace = draft.before.clone();
            self.mode = super::Mode::Normal;
        }
        self.workspace.last_visit = chrono::Utc::now().timestamp();
        let barrier = self.flush_session();
        let configuration = self.finish_configuration_preferences();
        let save = self.save_session();
        let flush = self.flush_session();
        finish_writes([
            recommendations,
            draft,
            barrier,
            configuration,
            save,
            flush,
        ])
    }
}

fn finish_writes<const N: usize>(results: [Result<()>; N]) -> Result<()> {
    let failures = results
        .into_iter()
        .filter_map(Result::err)
        .map(|error| format!("{error:#}"))
        .collect::<Vec<_>>();
    anyhow::ensure!(failures.is_empty(), "{}", failures.join("; "));
    Ok(())
}
