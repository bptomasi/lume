//! Cached article-local annotation ordering, distinct from creation order.
//!
//! Current-source quotes relocate only through the existing exact/unique
//! contract. Other sources and unresolved/general notes stay explicitly labeled.
//! Selecting a row never writes; jumping reuses source-bound reader history.

use anyhow::{Context, Result};
use crossterm::event::KeyCode;

use super::{Action, App, Focus, ReaderLocation, passages::locate_quote};

/// Position known for an annotation in the currently selected article.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AnnotationPosition {
    /// Safely resolved in the currently displayed source.
    Located {
        /// UTF-8 source byte anchor, independent of terminal width.
        anchor: usize,
        /// Approximate position within this source, in percent.
        percent: usize,
    },
    /// Stored location in the alternate source; validated when jumping.
    OtherSource {
        /// Original source anchor; not a verified current-source location.
        anchor: usize,
    },
    /// The saved quote no longer has a safe position in the current source.
    Unplaced,
    /// A private comment without a captured passage.
    General,
}

/// Derived annotation row pointing into the article's cached comment list.
#[derive(Clone, Debug)]
pub struct AnnotationEntry {
    /// Durable annotation identity.
    pub id: i64,
    /// Index into `App::comments`; rebuilt whenever those comments reload.
    pub comment_index: usize,
    /// Resolved position or an explicit non-positioned category.
    pub position: AnnotationPosition,
}

impl AnnotationEntry {
    fn order(&self) -> (u8, usize, i64) {
        let (group, position) = match self.position {
            AnnotationPosition::Located { anchor, .. } => (0, anchor),
            AnnotationPosition::OtherSource { anchor } => (1, anchor),
            AnnotationPosition::Unplaced => (2, 0),
            AnnotationPosition::General => (3, 0),
        };
        (group, position, self.id)
    }
}

impl App {
    pub(super) fn select_first_annotation(&mut self) {
        if let Some(entry) = self.annotations.first() {
            self.comment_selected = entry.comment_index;
        }
    }

    pub(super) fn rebuild_annotations(&mut self) {
        let mut relocated = std::collections::HashMap::new();
        self.annotations = self
            .comments
            .iter()
            .enumerate()
            .map(|(index, note)| AnnotationEntry {
                id: note.id,
                comment_index: index,
                position: self.annotation_position(note, &mut relocated),
            })
            .collect();
        self.annotations.sort_by_key(AnnotationEntry::order);
    }

    fn annotation_position<'a>(
        &self,
        note: &'a crate::model::Comment,
        relocated: &mut std::collections::HashMap<
            &'a str,
            Option<std::ops::Range<usize>>,
        >,
    ) -> AnnotationPosition {
        if note.quote.is_empty() {
            return AnnotationPosition::General;
        }
        if note.source != self.content_source {
            return AnnotationPosition::OtherSource {
                anchor: note.anchor,
            };
        }
        let anchor = if note.source_revision == self.content_revision {
            note.anchor
        } else {
            self.body.len()
        };
        let exact = self
            .body
            .get(anchor..)
            .is_some_and(|tail| tail.starts_with(&note.quote));
        let range = if exact {
            Some(anchor..anchor + note.quote.len())
        } else {
            relocated
                .entry(note.quote.as_str())
                .or_insert_with(|| {
                    locate_quote(&self.body, &note.quote, self.body.len())
                })
                .clone()
        };
        match range {
            Some(range) => AnnotationPosition::Located {
                anchor: range.start,
                percent: range.start.saturating_mul(100)
                    / self.body.len().max(1),
            },
            None => AnnotationPosition::Unplaced,
        }
    }

    /// Selected position in the sorted annotation pane, if any notes exist.
    pub fn annotation_selected(&self) -> Option<usize> {
        if self.annotations.is_empty() {
            return None;
        }
        Some(
            self.annotations
                .iter()
                .position(|entry| entry.comment_index == self.comment_selected)
                .unwrap_or(0),
        )
    }

    pub(super) fn move_annotation(&mut self, forward: bool, distance: usize) {
        let Some(selected) = self.annotation_selected() else {
            return;
        };
        let next = if forward {
            selected
                .saturating_add(distance)
                .min(self.annotations.len() - 1)
        } else {
            selected.saturating_sub(distance)
        };
        self.comment_selected = self.annotations[next].comment_index;
    }

    pub(super) fn jump_annotation(&mut self, position: usize) -> Result<()> {
        let entry = self.annotations.get(position).context("Select a note")?;
        let note = self.comments[entry.comment_index].clone();
        self.comment_selected = entry.comment_index;
        self.workspace.show_reader = true;
        if note.quote.is_empty() {
            self.focus = Focus::Reader;
            let marker = format!("[{}] ", self.comment_selected + 1);
            if let Some(row) = self
                .reader_lines
                .iter()
                .position(|line| line.starts_with(&marker))
            {
                self.reader_cursor = row;
                self.reader_column = 0;
                self.ensure_cursor_visible();
            }
            return Ok(());
        }
        let article = self.current().context("Select an article")?.id;
        self.jump_reader(ReaderLocation {
            article,
            source: note.source,
            revision: note.source_revision,
            anchor: note.anchor,
            quote: Some(note.quote),
        })?;
        if let Some(index) =
            self.comments.iter().position(|item| item.id == note.id)
        {
            self.comment_selected = index;
        }
        Ok(())
    }

    pub(super) fn annotation_key(
        &mut self,
        code: KeyCode,
    ) -> Result<Option<Action>> {
        if self.focus != Focus::Annotations {
            return Ok(None);
        }
        match code {
            KeyCode::Enter => {
                let selected =
                    self.annotation_selected().context("No annotations yet")?;
                self.jump_annotation(selected)?;
            }
            KeyCode::Char('n') => self.begin_reader_note()?,
            KeyCode::Char('e') => self.edit_comment(true)?,
            KeyCode::Char('D') | KeyCode::Delete => {
                return self.command("delete-comment").map(Some);
            }
            KeyCode::Esc => self.focus_pane(Focus::Reader)?,
            _ => return Ok(None),
        }
        Ok(Some(Action::None))
    }
}
