//! Cached local search and source-bound jump history.
use super::ReaderState;
use crate::app::ReaderLocation;
use crate::reader::index::DocumentIndex;
use anyhow::{Context, Result, ensure};
use std::{collections::VecDeque, ops::Range};

/// Reader-only navigation caches and bounded jump history.
#[derive(Default)]
pub struct ReaderTools {
    /// Width-independent CommonMark structure.
    pub index: DocumentIndex,
    /// Literal case-sensitive search, separate from library filtering.
    pub needle: String,
    /// At most 4,096 readable-text matches.
    pub hits: Vec<Range<usize>>,
    back: VecDeque<ReaderLocation>,
    forward: Vec<ReaderLocation>,
}

impl ReaderState {
    pub(super) fn refresh_reader_search(&mut self) {
        self.projection.tools.hits = if self.projection.tools.needle.is_empty()
        {
            Vec::new()
        } else {
            self.projection.text[..self.projection.article_end]
                .match_indices(&self.projection.tools.needle)
                .take(4096)
                .map(|(start, text)| start..start + text.len())
                .collect()
        };
    }

    pub(in crate::app) fn reader_location(&self) -> Result<ReaderLocation> {
        let offset = self.cursor_offset();
        let end = crate::reader::cursor::next(&self.projection.text, offset);
        let anchor =
            crate::reader::to_source(&self.projection.mappings, offset..end)
                .map(|range| range.start)
                .unwrap_or_else(|| {
                    self.projection
                        .source_ranges
                        .get(self.viewport.row)
                        .map_or(0, |range| range.start)
                });
        Ok(ReaderLocation {
            article: self.reading_article().context("Select an article")?,
            source: self.publisher.source,
            revision: self.publisher.revision,
            anchor,
            quote: self
                .publisher
                .body
                .get(anchor..)
                .map(|tail| tail.chars().take(64).collect()),
        })
    }

    pub(in crate::app) fn remember_reader_jump(&mut self) {
        if let Ok(location) = self.reader_location() {
            self.projection.tools.back.push_back(location);
            if self.projection.tools.back.len() > 128 {
                self.projection.tools.back.pop_front();
            }
            self.projection.tools.forward.clear();
        }
    }

    pub(in crate::app) fn resolve_reader_location(
        &self,
        target: &ReaderLocation,
    ) -> Result<usize> {
        if let Some(quote) = &target.quote {
            let anchor = if self.publisher.revision == target.revision {
                target.anchor
            } else {
                self.publisher.body.len()
            };
            return crate::reader::locate_quote(
                &self.publisher.body,
                quote,
                anchor,
            )
            .map(|range| range.start)
            .context(
                "Passage changed or is ambiguous; original quote remains saved",
            );
        }
        ensure!(
            self.publisher.revision == target.revision,
            "Passage revision changed"
        );
        Ok(target.anchor)
    }
}

impl ReaderState {
    pub(in crate::app) fn search_reader(&mut self, needle: &str) {
        self.projection.tools.needle = needle.to_owned();
        self.refresh_reader_search();
    }

    pub(in crate::app) fn record_reader_jump(
        &mut self,
        previous: ReaderLocation,
    ) {
        self.projection.tools.back.push_back(previous);
        if self.projection.tools.back.len() > 128 {
            self.projection.tools.back.pop_front();
        }
        self.projection.tools.forward.clear();
    }

    pub(in crate::app) fn history_target(
        &self,
        forward: bool,
    ) -> Option<ReaderLocation> {
        if forward {
            self.projection.tools.forward.last().cloned()
        } else {
            self.projection.tools.back.back().cloned()
        }
    }

    pub(in crate::app) fn advance_history(
        &mut self,
        current: ReaderLocation,
        forward: bool,
    ) {
        if forward {
            self.projection.tools.forward.pop();
            self.projection.tools.back.push_back(current);
        } else {
            self.projection.tools.back.pop_back();
            self.projection.tools.forward.push(current);
        }
    }
}
