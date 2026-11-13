//! Local reader search and captured source jumps, independent of library search.

use crate::app::commands::routing;

use anyhow::{Context, Result, ensure};

use crate::model::ContentSource;

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

impl App {
    pub(super) fn reader_tools_command(
        &mut self,
        command: routing::ReaderToolsCommand,
        argument: &str,
    ) -> Result<Action> {
        use routing::ReaderToolsCommand as C;

        match command {
            C::ExplainSearch => {
                let explanation = self.explain_active_search(argument)?;
                self.show_document(
                    "Search explanation · Esc closes",
                    &explanation,
                    None,
                );
            }
            C::Find => self.search_reader(argument)?,
            C::FindNext => self.next_reader_match(true)?,
            C::FindPrev => self.next_reader_match(false)?,
            C::Note => self.begin_reader_note()?,
            C::EditNote => self.edit_context_note()?,
            C::Annotate => self.annotate_selection()?,
            C::Toc => self.show_outline()?,
            C::Highlights => self.show_highlights(argument)?,
            C::JumpBack => self.jump_history(false)?,
            C::JumpForward => self.jump_history(true)?,
            C::CopyCode => return self.copy_code(),
            C::Tables => self.show_tables()?,
        }
        Ok(Action::None)
    }

    fn explain_active_search(&self, argument: &str) -> Result<String> {
        if !argument.is_empty() {
            return crate::search::explain(argument);
        }
        let mut explanation = format!(
            "Live search:\n{}",
            crate::search::explain(self.nav.search())?
        );
        if let super::View::Smart(id) = *self.nav.view() {
            let list = self
                .nav
                .library()
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
        self.nav
            .library()
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
        self.reader.search_reader(needle);
        self.next_reader_match(true)
    }

    pub(super) fn next_reader_match(&mut self, forward: bool) -> Result<()> {
        let hits = &self.reader.view().reader_tools.hits;
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
            self.reader.view().reader_tools.hits.len()
        );
        Ok(())
    }

    fn show_outline(&mut self) -> Result<()> {
        let location = self.reader_location()?;
        let choices = self
            .reader
            .view()
            .reader_tools
            .index
            .headings
            .iter()
            .map(|heading| {
                let mut target = location.clone();
                target.anchor = heading.range.start;
                target.quote = self
                    .reader
                    .view()
                    .body
                    .get(heading.range.clone())
                    .map(str::to_owned);
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
        let query = query.to_lowercase();
        for comment in self.reader.view().comments {
            if comment.quote.is_empty()
                || !format!("{} {}", comment.quote, comment.body)
                    .to_lowercase()
                    .contains(&query)
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

    pub(super) fn jump_reader(&mut self, target: ReaderLocation) -> Result<()> {
        let previous = self.reader_location()?;
        self.restore_reader_location(target)?;
        self.reader.record_reader_jump(previous);
        Ok(())
    }

    fn restore_reader_location(
        &mut self,
        target: ReaderLocation,
    ) -> Result<()> {
        if self
            .current()
            .is_some_and(|article| article.id == target.article)
            && self.reader.source() == target.source
            && self.reader.revision() == target.revision
            && *self.nav.view() != super::View::Annotated
        {
            return self.apply_reader_location(target);
        }
        if self
            .current()
            .is_none_or(|article| article.id != target.article)
            || *self.nav.view() == super::View::Annotated
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
            self.reader.source() == target.source,
            "Requested source is unavailable; original quote remains saved"
        );
        let anchor = self.resolve_reader_location(&target)?;
        self.reader.cancel_restore();
        self.restore_anchor(anchor);
        if let Some(range) = crate::reader::to_display(
            self.reader.view().reader_mappings,
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

    pub(super) fn jump_history(&mut self, forward: bool) -> Result<()> {
        let current = self.reader_location()?;
        let target = self
            .reader
            .history_target(forward)
            .context("No further reader jump history")?;
        self.restore_reader_location(target)?;
        self.reader.advance_history(current, forward);
        Ok(())
    }

    fn copy_code(&self) -> Result<Action> {
        let anchor = self.reader_location()?.anchor;
        let blocks = &self.reader.view().reader_tools.index.code;
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
        let tables = &self.reader.view().reader_tools.index.tables;
        ensure!(!tables.is_empty(), "No Markdown tables in this article");
        let text = if self.reader.view().table_rows.len() == tables.len()
            && !self.reader_stale()
        {
            crate::reader::table_view::from_lines(
                self.reader
                    .view()
                    .table_rows
                    .iter()
                    .flat_map(|range| {
                        self.reader
                            .view()
                            .reader_lines
                            .get(range.clone())
                            .into_iter()
                            .flatten()
                    })
                    .map(String::as_str),
            )
        } else {
            let source = tables
                .iter()
                .filter_map(|range| self.reader.view().body.get(range.clone()))
                .collect::<Vec<_>>()
                .join("\n\n");
            crate::reader::table_view::render_with_options(
                &source,
                self.formula_options(),
            )
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
            self.reader.view().reader_tools.needle.clone(),
        );
    }

    pub(super) fn reader_location(&self) -> Result<ReaderLocation> {
        self.guard_stale_command(crate::command::Command::JumpBack)?;
        self.reader.reader_location()
    }

    pub(super) fn remember_reader_jump(&mut self) {
        if !self.reader_stale() {
            self.reader.remember_reader_jump();
        }
    }

    fn resolve_reader_location(
        &self,
        target: &ReaderLocation,
    ) -> Result<usize> {
        self.reader.resolve_reader_location(target)
    }
}
