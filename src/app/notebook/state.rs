//! Query and page publication preserve exact selected-note context.

use super::{NoteContext, NotePage, NoteSummary, Notebook, Origin, readers};
use anyhow::Result;

impl Notebook {
    pub(super) fn start(
        path: &std::path::Path,
        query: &str,
        article: Option<i64>,
        origin: Origin,
    ) -> Result<Self> {
        Ok(Self {
            query: query.into(),
            page: NotePage::default(),
            offset: 0,
            selected: 0,
            collections: Vec::new(),
            collection: 0,
            context: None,
            scroll: 0,
            link_selected: 0,
            recent: Vec::new(),
            recent_generation: 0,
            marked: std::collections::BTreeSet::new(),
            loading: false,
            context_loading: false,
            error: String::new(),
            article,
            width: 72,
            pending_delete: None,
            origin,
            readers: readers::Readers::start(path)?,
            history: Vec::new(),
            loaded_query: String::new(),
            loaded_offset: 0,
            loaded_article: article,
        })
    }

    /// Selected note preview, including while another read is pending.
    pub fn current(&self) -> Option<&NoteSummary> {
        if self.loaded_query != self.query
            || self.loaded_offset != self.offset
            || self.loaded_article != self.article
        {
            return None;
        }
        self.page.notes.get(self.selected)
    }

    /// Context is available only for the exact currently selected note.
    pub fn current_context(&self) -> Option<&NoteContext> {
        let id = self.current()?.id;
        self.context
            .as_ref()
            .filter(|item| item.note.comment.id == id)
    }

    pub(super) fn refresh(&mut self) -> Result<()> {
        crate::search::notes::validate(&self.query)?;
        self.readers.list.submit(readers::ListRequest {
            text: self.query.clone(),
            article: self.article,
            offset: self.offset,
            recent_generation: self.recent_generation,
        });
        self.loading = true;
        self.error.clear();
        Ok(())
    }

    pub(in crate::app) fn install_recent(&mut self, recent: Vec<String>) {
        // A late list snapshot must not overwrite an acknowledged history edit.
        self.recent_generation = self.recent_generation.wrapping_add(1);
        self.recent = recent;
    }

    pub(super) fn install_listing(&mut self, listing: readers::Listing) {
        let preserve = self.query == self.loaded_query
            && self.offset == self.loaded_offset;
        let id = self.current().filter(|_| preserve).map(|note| note.id);
        self.loaded_query.clone_from(&self.query);
        self.loaded_offset = listing.offset;
        self.loaded_article = self.article;
        self.page = listing.page;
        self.offset = listing.offset;
        self.collections = listing.collections;
        self.collection = self.collection.min(self.collections.len() + 3);
        if self.recent_generation == listing.recent_generation {
            self.recent = listing.recent;
        }
        self.selected = id
            .and_then(|id| {
                self.page.notes.iter().position(|note| note.id == id)
            })
            .unwrap_or(
                self.selected.min(self.page.notes.len().saturating_sub(1)),
            );
        let scroll = self.scroll;
        self.read_context();
        if self.current().is_some_and(|note| Some(note.id) == id) {
            self.scroll = scroll;
        }
    }

    pub(super) fn read_context(&mut self) {
        let Some(id) = self.current().map(|note| note.id) else {
            self.context = None;
            self.context_loading = false;
            self.readers.context.cancel();
            return;
        };
        self.readers.context.submit((id, self.width));
        self.context_loading = true;
        if self
            .context
            .as_ref()
            .is_none_or(|context| context.note.comment.id != id)
        {
            self.scroll = 0;
            self.link_selected = 0;
        }
    }

    pub(super) fn poll(&mut self) -> Result<bool> {
        let mut changed = false;
        if let Some(result) = self.readers.list.poll() {
            self.loading = false;
            changed = true;
            self.install_listing(result?);
        }
        if let Some(result) = self.readers.context.poll() {
            self.context_loading = false;
            self.context = Some(result?);
            changed = true;
        }
        Ok(changed)
    }

    pub(super) fn set_query(&mut self, query: &str) -> Result<()> {
        if self.query == query {
            return Ok(());
        }
        crate::search::notes::validate(query)?;
        self.query = query.into();
        self.offset = 0;
        self.selected = 0;
        self.refresh()
    }

    pub(super) fn follow_reference(&mut self, id: i64) -> Result<()> {
        if self.history.len() == 128 {
            self.history.remove(0);
        }
        self.history
            .push((self.query.clone(), self.offset, self.selected));
        self.set_query(&format!("id:{id}"))
    }

    pub(super) fn restore_history(&mut self) -> Result<()> {
        if let Some((query, offset, selected)) = self.history.pop() {
            self.query = query;
            self.offset = offset;
            self.selected = selected;
            self.refresh()?;
        }
        Ok(())
    }

    pub(super) fn resize_context(&mut self, width: usize) {
        let width = width.clamp(10, 512);
        if self.width != width {
            self.width = width;
            let scroll = self.scroll;
            self.read_context();
            self.scroll = scroll;
        }
    }

    pub(super) fn clamp_viewport(&mut self, limit: Option<usize>) {
        if let Some(context) = self.current_context() {
            let links = context.backlinks.len() + context.references.len();
            self.link_selected =
                self.link_selected.min(links.saturating_sub(1));
            if let Some(limit) = limit {
                self.scroll = self.scroll.min(limit);
            }
        }
    }
}
