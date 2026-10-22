//! Temporary web readings never borrow an article's mutation identity.
//!
//! HTTP, extraction and layout run on a latest-request lane. Back/forward owns
//! bounded page snapshots; the underlying article and its cursor stay untouched.

mod keys;
mod page;
pub use page::{WebDocument, WebPage};

use std::{collections::VecDeque, sync::Arc};

use anyhow::{Context, Result, ensure};

use super::{Action, App, Mode};
use crate::latest::Latest;

enum Request {
    Open {
        address: String,
        width: usize,
        online: bool,
        fragment: Option<String>,
    },
    Reflow {
        document: Arc<WebDocument>,
        width: usize,
        anchor: usize,
    },
}

#[derive(Default)]
pub(super) struct WebReading {
    pub(super) active: bool,
    worker: Option<Latest<Request, WebPage>>,
    current: Option<WebPage>,
    back: VecDeque<WebPage>,
    forward: Vec<WebPage>,
    requested_width: usize,
    opening: bool,
    mouse_anchor: Option<usize>,
}

impl App {
    /// Permit network work only when the runtime explicitly enables it.
    pub fn set_network_enabled(&mut self, enabled: bool) {
        self.network_enabled = enabled && !self.demo;
    }

    /// Current read-only linked page, independent of the library article.
    pub fn web_page(&self) -> Option<&WebPage> {
        self.web.current.as_ref()
    }

    /// Whether a linked reading or its overlays own the screen.
    pub fn web_active(&self) -> bool {
        self.web.active
    }

    pub(super) fn open_web_page(&mut self, address: &str) -> Result<()> {
        let mut url = crate::source::link_url(address)?;
        let fragment = url.fragment().map(str::to_owned);
        url.set_fragment(None);
        self.ensure_web_worker()?;
        let width = self.web_width();
        self.web.requested_width = width;
        self.web
            .worker
            .as_mut()
            .context("Web worker unavailable")?
            .submit(Request::Open {
                address: url.to_string(),
                width,
                online: self.network_enabled,
                fragment,
            });
        self.web.active = true;
        self.web.opening = true;
        self.mode = Mode::Web;
        self.status = "Opening linked page… · Esc cancels".into();
        Ok(())
    }

    fn ensure_web_worker(&mut self) -> Result<()> {
        if self.web.worker.is_some() {
            return Ok(());
        }
        let path = self.store.database_path()?;
        let mut connection = None;
        self.web.worker = Some(Latest::start(
            "lume-web-reader",
            move |request| match request {
                Request::Open {
                    address,
                    width,
                    online,
                    fragment,
                } => {
                    let document =
                        page::load(&mut connection, &path, &address, online)?;
                    let mut page =
                        WebPage::prepare(Arc::new(document), width, 0);
                    if let Some(fragment) = fragment {
                        page.jump_fragment(&fragment)?;
                    }
                    Ok(page)
                }
                Request::Reflow {
                    document,
                    width,
                    anchor,
                } => Ok(WebPage::prepare(document, width, anchor)),
            },
        )?);
        Ok(())
    }

    pub(super) fn web_width(&self) -> usize {
        usize::from(self.screen.width.saturating_sub(6))
            .min(self.workspace.reading_width.into())
            .max(1)
    }

    /// Apply current linked-page results and request reflow after resizing.
    ///
    /// # Errors
    /// Reports extraction/network errors without altering the library reading.
    pub fn poll_web(&mut self) -> Result<bool> {
        if !self.web.active || !matches!(self.mode, Mode::Web) {
            return Ok(false);
        }
        let result = self.web.worker.as_mut().and_then(Latest::poll);
        let changed = result.is_some();
        if let Some(result) = result {
            let reflow = !self.web.opening;
            self.web.opening = false;
            let mut page = result?;
            if reflow && let Some(current) = &mut self.web.current {
                page.inherit_view(current);
            }
            page.reveal(usize::from(self.screen.height.saturating_sub(6)));
            if let Some(previous) = self.web.current.take()
                && previous.document.url != page.document.url
            {
                self.web.back.push_back(previous);
                self.web.forward.clear();
                while self.web.back.len() > 8 {
                    self.web.back.pop_front();
                }
            }
            self.web.current = Some(page);
            self.status =
                "Linked page · Ctrl-o back · :save-page keeps it".into();
        }
        self.reflow_web();
        Ok(changed)
    }

    fn reflow_web(&mut self) {
        let width = self.web_width();
        if self.web.opening || self.web.requested_width == width {
            return;
        }
        let Some(page) = &self.web.current else {
            return;
        };
        if let Some(worker) = &mut self.web.worker {
            worker.submit(Request::Reflow {
                document: page.document.clone(),
                width,
                anchor: page.source_anchor(),
            });
            self.web.requested_width = width;
        }
    }

    /// Whether linked-page HTTP or layout is still in progress.
    pub fn web_pending(&self) -> bool {
        self.web.active && self.web.worker.as_ref().is_some_and(Latest::pending)
    }

    pub(super) fn restore_web_mode(&mut self) {
        if self.web.active && matches!(self.mode, Mode::Normal) {
            self.mode = Mode::Web;
        }
    }

    pub(super) fn web_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "open-link" => {
                let address = if self.web.active {
                    self.web_page().and_then(WebPage::cursor_link)
                } else {
                    self.cursor_link()
                }
                .context("Place the cursor on a link")?
                .to_owned();
                self.open_web_page(&address)?;
            }
            "open-page" => self.open_web_page(argument)?,
            "save-page" => {
                let page = self
                    .web
                    .current
                    .as_ref()
                    .context("Open a linked page first")?;
                ensure!(self.web.active, "Open a linked page first");
                let document = page.document.clone();
                self.save_web_document(document)?;
            }
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    /// Clear the active copied selection without changing a suspended article.
    pub fn clear_copied_selection(&mut self) {
        if self.web.active {
            if let Some(page) = &mut self.web.current {
                page.selection = None;
            }
        } else {
            self.selection = None;
        }
    }

    pub(super) fn close_web(&mut self) {
        if let Some(worker) = &mut self.web.worker {
            worker.cancel();
        }
        self.web.mouse_anchor = None;
        self.web.active = false;
        self.web.opening = false;
        self.web.current = None;
        self.web.back.clear();
        self.web.forward.clear();
        self.mode = Mode::Normal;
    }

    pub(super) fn finish_web_save(
        &mut self,
        article: i64,
        url: &str,
    ) -> Result<()> {
        if self.web.active
            && self
                .web
                .current
                .as_ref()
                .is_some_and(|page| page.document.url == url)
        {
            self.close_web();
            self.reload()?;
            self.jump_article(article)?;
        }
        self.status = "Page saved · Saved pages folder".into();
        Ok(())
    }

    fn save_web_document(&mut self, document: Arc<WebDocument>) -> Result<()> {
        if let Some(writer) = &mut self.personal_writes {
            writer.submit(super::personal_tasks::Task::SavePage(document))?;
            self.status = "Saving linked page…".into();
        } else {
            let article = self.store.save_web_document(&document)?;
            self.close_web();
            self.reload()?;
            self.jump_article(article)?;
        }
        Ok(())
    }
}
