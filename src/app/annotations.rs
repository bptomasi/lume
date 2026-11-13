//! Cached article-local annotation ordering, distinct from creation order.
//!
//! Current-source quotes relocate only through the existing exact/unique
//! contract. Other sources and unresolved/general notes stay explicitly labeled.
//! Selecting a row never writes; jumping reuses source-bound reader history.

use anyhow::{Context, Result};
use crossterm::event::KeyCode;

use super::{Action, App, Focus, ReaderLocation};

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
    /// Index into `ReaderView::comments`; rebuilt whenever those comments reload.
    pub comment_index: usize,
    /// Resolved position or an explicit non-positioned category.
    pub position: AnnotationPosition,
}

impl AnnotationEntry {
    pub(super) fn order(&self) -> (u8, usize, i64) {
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
    pub(super) fn jump_annotation(&mut self, position: usize) -> Result<()> {
        let entry = self
            .reader
            .view()
            .annotations
            .get(position)
            .context("Select a note")?;
        let note = self.reader.view().comments[entry.comment_index].clone();
        self.reader.select_comment(entry.comment_index);
        self.layout.show_pane(crate::model::Pane::Reader, true);
        if note.quote.is_empty() {
            self.layout.focus_pane(Focus::Reader);
            self.reader.reveal_selected_comment();
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
        if let Some(index) = self
            .reader
            .view()
            .comments
            .iter()
            .position(|item| item.id == note.id)
        {
            self.reader.select_comment(index);
        }
        Ok(())
    }

    pub(super) fn annotation_key(
        &mut self,
        code: KeyCode,
    ) -> Result<Option<Action>> {
        if self.layout.focus() != Focus::Annotations {
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
