//! Cached presentation and narrow terminal adapter notifications.

use super::{App, Focus, Mode, ReaderImages};
use crate::model::{ArticleTarget, Workspace};

impl App {
    /// Borrow coherent geometry inputs without allocating or performing I/O.
    pub fn layout(&self) -> super::LayoutView<'_> {
        self.layout.view()
    }

    /// Preferences used by geometry and rendering, borrowed without I/O.
    pub fn workspace(&self) -> &Workspace {
        self.layout.workspace()
    }

    /// Pane currently receiving keyboard input.
    pub fn focus(&self) -> Focus {
        self.layout.focus()
    }

    /// Current modal draft or overlay, borrowed without worker authority.
    pub fn mode(&self) -> &Mode {
        self.interaction.mode()
    }

    /// Most recent recoverable failure or operation status.
    pub fn status(&self) -> &str {
        self.interaction.status()
    }

    /// Publish an adapter result after worker admission or completion.
    pub fn report_status(&mut self, status: String) {
        self.interaction.report(status);
    }

    /// Whether the runtime has admitted a feed collection or discovery pass.
    pub fn refreshing(&self) -> bool {
        self.network.collecting()
    }

    /// Record a collection admitted by the runtime worker.
    pub fn collection_started(&mut self) {
        self.network.collection_started();
    }

    /// Release collection admission after its terminal completion.
    pub fn collection_finished(&mut self) {
        self.network.collection_finished();
    }

    /// Whether this is the isolated demonstration library.
    pub fn demo(&self) -> bool {
        self.network.demo()
    }

    /// Inspect prepared images without taking loading or cancellation authority.
    pub fn images(&self) -> &ReaderImages {
        &self.images
    }

    /// Captured identity of the outstanding article request.
    pub fn article_fetching(&self) -> Option<&ArticleTarget> {
        self.article_requests.fetching()
    }

    /// Current guide scroll offset.
    pub fn help_scroll(&self) -> usize {
        self.help().scroll
    }

    /// Focus and expand a pane while retaining its modal and source selection.
    pub fn activate_pane(&mut self, pane: Focus) {
        self.layout.focus_pane(pane);
    }
}
