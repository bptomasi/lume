//! Enter follows the cursor's exact link occurrence without parsing on input.
//!
//! Local jumps reuse source anchors and history without reloading SQLite.
//! External destinations open a read-only linked page on a background lane.

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};

use super::{Action, App, Focus};

impl App {
    pub(super) fn reader_link_key(
        &mut self,
        key: KeyEvent,
    ) -> Result<Option<Action>> {
        if key.code != KeyCode::Enter
            || self.focus != Focus::Reader
            || self.selection.is_some()
            || self.captured_image().is_ok()
        {
            return Ok(None);
        }
        let Some(destination) = self.cursor_link().map(str::to_owned) else {
            return Ok(None);
        };
        if key.kind == KeyEventKind::Repeat {
            return Ok(Some(Action::None));
        }
        self.follow_reader_link(&destination).map(Some)
    }

    /// Supported link under the reader cursor, excluding appended private notes.
    pub fn cursor_link(&self) -> Option<&str> {
        if self.focus != Focus::Reader || self.reader_stale() {
            return None;
        }
        let offset = self.cursor_offset();
        if offset >= self.article_text_end {
            return None;
        }
        let end = crate::reader::cursor::next(&self.reader_text, offset);
        let source =
            crate::reader::to_source(&self.reader_mappings, offset..end)?;
        self.reader_tools
            .index
            .navigation
            .link_at(source.start)
            .map(|link| link.destination.as_str())
    }

    fn follow_reader_link(&mut self, destination: &str) -> Result<Action> {
        let fragment = if let Some(fragment) = destination.strip_prefix('#') {
            fragment.to_owned()
        } else {
            let url = crate::source::link_url(destination)?;
            if !self.link_is_current_page(&url) || url.fragment().is_none() {
                self.open_web_page(url.as_str())?;
                return Ok(Action::None);
            }
            url.fragment().unwrap_or_default().to_owned()
        };
        let anchor = self
            .reader_tools
            .index
            .navigation
            .anchor(&fragment)
            .context("This section is absent from the cached article · o opens the original")?;
        let mut location = self.reader_location()?;
        location.anchor = anchor;
        location.quote = None;
        self.remember_reader_jump();
        self.apply_reader_location(location)?;
        self.status = "Section opened · Ctrl-o back · Ctrl-i forward".into();
        Ok(Action::None)
    }

    fn link_is_current_page(&self, url: &url::Url) -> bool {
        let mut page = url.clone();
        page.set_fragment(None);
        self.current()
            .map(|article| article.url.as_str())
            .into_iter()
            .chain(
                self.document
                    .as_ref()
                    .filter(|document| {
                        self.current()
                            .is_some_and(|article| article.url == document.url)
                    })
                    .map(|document| document.final_url.as_str()),
            )
            .any(|address| {
                crate::source::feed_url(address).ok().as_ref() == Some(&page)
            })
    }
}
