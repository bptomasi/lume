//! Local reader search and captured source jumps, independent of library search.

use std::{collections::VecDeque, ops::Range};

use anyhow::{Context, Result, ensure};

use crate::{model::ContentSource, reader::index::DocumentIndex};

use super::{Action, App, Choice, ChoiceAction, Focus, InputKind};

/// A source-bound location; reflow cannot turn it into a different passage.
#[derive(Clone, Debug)]
pub struct ReaderLocation {
    /// Captured article identity.
    pub article: i64,
    /// Feed or Full source being read.
    pub source: ContentSource,
    /// Full revision, or zero for Feed.
    pub revision: i64,
    /// Source byte offset, independent of display width.
    pub anchor: usize,
    /// Optional immutable highlight text, required for safe revision relocation.
    pub quote: Option<String>,
}

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

impl App {
    pub(super) fn reader_tools_command(
        &mut self,
        verb: &str,
        argument: &str,
    ) -> Result<Option<Action>> {
        match verb {
            "explain-search" => {
                let explanation = self.explain_active_search(argument)?;
                self.show_document(
                    "Search explanation · Esc closes",
                    &explanation,
                    None,
                );
            }
            "find" => self.search_reader(argument)?,
            "find-next" => self.next_reader_match(true)?,
            "find-prev" => self.next_reader_match(false)?,
            "note" => self.begin_reader_note()?,
            "edit-note" => self.edit_context_note()?,
            "annotate" => self.annotate_selection()?,
            "toc" => self.show_outline()?,
            "highlights" => self.show_highlights(argument)?,
            "jump-back" => self.jump_history(false)?,
            "jump-forward" => self.jump_history(true)?,
            "copy-code" => return self.copy_code().map(Some),
            "tables" => self.show_tables()?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }

    fn explain_active_search(&self, argument: &str) -> Result<String> {
        if !argument.is_empty() {
            return crate::search::explain(argument);
        }
        let mut explanation =
            format!("Live search:\n{}", crate::search::explain(&self.search)?);
        if let super::View::Smart(id) = self.view {
            let list = self
                .library
                .smart_lists
                .iter()
                .find(|list| list.id == id)
                .context("Saved search no longer exists")?;
            explanation.push_str(&self.explain_saved_list(list)?);
        }
        Ok(explanation)
    }

    fn explain_saved_list(
        &self,
        list: &crate::model::SmartList,
    ) -> Result<String> {
        let filter = &list.filter;
        let mut explanation = format!(
            "\n\nSaved list: {}\nAll these criteria must also match:\n{}",
            list.name,
            crate::search::explain(&filter.text)?
        );
        if filter.unread {
            explanation.push_str("\nOnly unread articles");
        }
        if filter.starred {
            explanation.push_str("\nOnly favorites");
        }
        if let Some(days) = filter.days {
            explanation
                .push_str(&format!("\nPublication age: at most {days} days"));
        }
        if let Some(feed) = filter.feed {
            explanation
                .push_str(&format!("\nSource: {}", self.source_name(feed)));
        }
        if !filter.feeds.is_empty() {
            let sources = filter
                .feeds
                .iter()
                .map(|feed| self.source_name(*feed))
                .collect::<Vec<_>>();
            explanation.push_str(&format!(
                "\nAny of these sources: {}",
                sources.join(", ")
            ));
        }
        Ok(explanation)
    }

    fn source_name(&self, id: i64) -> String {
        self.library
            .feeds
            .iter()
            .find(|feed| feed.id == id)
            .map_or_else(
                || format!("Archived source #{id}"),
                |feed| feed.title.clone(),
            )
    }

    pub(super) fn search_reader(&mut self, needle: &str) -> Result<()> {
        ensure!(needle.len() <= 1024, "Reader search exceeds 1 KiB");
        self.reader_tools.needle = needle.to_owned();
        self.refresh_reader_search();
        self.next_reader_match(true)
    }

    pub(super) fn refresh_reader_search(&mut self) {
        self.reader_tools.hits = if self.reader_tools.needle.is_empty() {
            Vec::new()
        } else {
            self.reader_text[..self.article_text_end]
                .match_indices(&self.reader_tools.needle)
                .take(4096)
                .map(|(start, text)| start..start + text.len())
                .collect()
        };
    }

    pub(super) fn next_reader_match(&mut self, forward: bool) -> Result<()> {
        let hits = &self.reader_tools.hits;
        let cursor = self.cursor_offset();
        let found = if forward {
            hits.iter().find(|hit| hit.start > cursor).or(hits.first())
        } else {
            hits.iter()
                .rev()
                .find(|hit| hit.start < cursor)
                .or(hits.last())
        };
        let hit = found
            .context("No reader matches · :find TEXT searches this article")?
            .start;
        self.remember_reader_jump();
        self.place_cursor(hit);
        self.ensure_cursor_visible();
        self.focus = Focus::Reader;
        self.workspace.show_reader = true;
        self.status = format!(
            "{} reader matches · :find-next / :find-prev",
            self.reader_tools.hits.len()
        );
        Ok(())
    }

    fn show_outline(&mut self) -> Result<()> {
        let location = self.reader_location()?;
        let choices = self
            .reader_tools
            .index
            .headings
            .iter()
            .map(|heading| {
                let mut target = location.clone();
                target.anchor = heading.range.start;
                target.quote =
                    self.body.get(heading.range.clone()).map(str::to_owned);
                Choice {
                    title: format!(
                        "{}{}",
                        "  ".repeat(heading.level.saturating_sub(1) as usize),
                        heading.title
                    ),
                    detail: format!(
                        "Heading {} · source byte {}",
                        heading.level, heading.range.start
                    ),
                    action: ChoiceAction::ReaderJump(target),
                }
            })
            .collect();
        self.inspect(choices)
    }

    fn show_highlights(&mut self, query: &str) -> Result<()> {
        let location = self.reader_location()?;
        let mut choices = Vec::new();
        for comment in &self.comments {
            if comment.quote.is_empty()
                || !format!("{} {}", comment.quote, comment.body)
                    .to_lowercase()
                    .contains(&query.to_lowercase())
            {
                continue;
            }
            let mut target = location.clone();
            target.source = comment.source;
            target.revision = comment.source_revision;
            target.anchor = comment.anchor;
            target.quote = Some(comment.quote.clone());
            choices.push(Choice {
                title: crate::content::label(&comment.quote, 120),
                detail: comment.body.clone(),
                action: ChoiceAction::ReaderJump(target),
            });
        }
        self.inspect(choices)
    }

    pub(super) fn reader_location(&self) -> Result<ReaderLocation> {
        self.guard_stale_command("reader-jump")?;
        let offset = self.cursor_offset();
        let end = crate::reader::cursor::next(&self.reader_text, offset);
        let anchor =
            crate::reader::to_source(&self.reader_mappings, offset..end)
                .map(|range| range.start)
                .unwrap_or_else(|| {
                    self.line_ranges
                        .get(self.reader_cursor)
                        .map_or(0, |range| range.start)
                });
        Ok(ReaderLocation {
            article: self.current().context("Select an article")?.id,
            source: self.content_source,
            revision: self.content_revision,
            anchor,
            quote: self
                .body
                .get(anchor..)
                .map(|tail| tail.chars().take(64).collect()),
        })
    }

    pub(super) fn remember_reader_jump(&mut self) {
        if self.reader_stale() {
            return;
        }
        if let Ok(location) = self.reader_location() {
            self.reader_tools.back.push_back(location);
            if self.reader_tools.back.len() > 128 {
                self.reader_tools.back.pop_front();
            }
            self.reader_tools.forward.clear();
        }
    }

    pub(super) fn jump_reader(&mut self, target: ReaderLocation) -> Result<()> {
        let previous = self.reader_location()?;
        self.restore_reader_location(target)?;
        self.reader_tools.back.push_back(previous);
        if self.reader_tools.back.len() > 128 {
            self.reader_tools.back.pop_front();
        }
        self.reader_tools.forward.clear();
        Ok(())
    }

    fn restore_reader_location(
        &mut self,
        target: ReaderLocation,
    ) -> Result<()> {
        if self
            .current()
            .is_none_or(|article| article.id != target.article)
            || self.view == super::View::Annotated
        {
            if self.navigation_worker.is_some() {
                self.workspace.content_source = target.source;
                self.jump_article(target.article)?;
                return self.on_navigation_arrival(
                    super::navigation_worker::Arrival::Reader(target),
                );
            }
            self.jump_article(target.article)?;
        }
        self.save_reading_position()?;
        self.workspace.content_source = target.source;
        self.load_current()?;
        self.apply_reader_location(target)
    }

    pub(super) fn apply_reader_location(
        &mut self,
        target: ReaderLocation,
    ) -> Result<()> {
        ensure!(
            self.content_source == target.source,
            "Requested source is unavailable; original quote remains saved"
        );
        let anchor = self.resolve_reader_location(&target)?;
        self.pending_anchor = None;
        self.restore_anchor(anchor);
        if let Some(range) = crate::reader::to_display(
            &self.reader_mappings,
            anchor..anchor.saturating_add(1),
        )
        .first()
        {
            self.place_cursor(range.start);
        }
        self.focus = Focus::Reader;
        self.ensure_cursor_visible();
        self.workspace.show_reader = true;
        Ok(())
    }

    fn resolve_reader_location(
        &self,
        target: &ReaderLocation,
    ) -> Result<usize> {
        if let Some(quote) = &target.quote {
            let anchor = if self.content_revision == target.revision {
                target.anchor
            } else {
                self.body.len()
            };
            return super::passages::locate_quote(&self.body, quote, anchor)
                .map(|range| range.start)
                .context("Passage changed or is ambiguous; original quote remains saved");
        }
        ensure!(
            self.content_revision == target.revision,
            "Passage revision changed"
        );
        Ok(target.anchor)
    }

    pub(super) fn jump_history(&mut self, forward: bool) -> Result<()> {
        let current = self.reader_location()?;
        let target = if forward {
            self.reader_tools.forward.last().cloned()
        } else {
            self.reader_tools.back.back().cloned()
        }
        .context("No further reader jump history")?;
        self.restore_reader_location(target)?;
        if forward {
            self.reader_tools.forward.pop();
            self.reader_tools.back.push_back(current);
        } else {
            self.reader_tools.back.pop_back();
            self.reader_tools.forward.push(current);
        }
        Ok(())
    }

    fn copy_code(&self) -> Result<Action> {
        let anchor = self.reader_location()?.anchor;
        let blocks = &self.reader_tools.index.code;
        let block = blocks
            .iter()
            .find(|block| block.range.contains(&anchor))
            .or_else(|| (blocks.len() == 1).then(|| &blocks[0]))
            .context("Place the reader cursor inside a code block")?;
        ensure!(
            block.text.len() <= 65_536,
            "Code block exceeds clipboard limit of 64 KiB"
        );
        Ok(Action::Copy(block.text.clone()))
    }

    fn show_tables(&mut self) -> Result<()> {
        let tables = &self.reader_tools.index.tables;
        ensure!(!tables.is_empty(), "No Markdown tables in this article");
        let text =
            if self.table_rows.len() == tables.len() && !self.reader_stale() {
                crate::reader::table_view::from_lines(
                    self.table_rows
                        .iter()
                        .flat_map(|range| {
                            self.reader_lines
                                .get(range.clone())
                                .into_iter()
                                .flatten()
                        })
                        .map(String::as_str),
                )
            } else {
                let source = tables
                    .iter()
                    .filter_map(|range| self.body.get(range.clone()))
                    .collect::<Vec<_>>()
                    .join("\n\n");
                crate::reader::table_view::render(&source)
            };
        self.show_document(
            "Tables · j/k vertical · h/l horizontal · Esc close",
            &text,
            None,
        );
        Ok(())
    }

    pub(super) fn begin_reader_search(&mut self) {
        self.begin_input(
            InputKind::ReaderSearch,
            self.reader_tools.needle.clone(),
        );
    }
}
