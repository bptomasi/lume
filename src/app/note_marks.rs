//! Width-specific note hits use cached readable bytes, never publisher guesses.
//!
//! Only safely resolved current-source quotations get passage markers. General
//! and unplaced notes remain actionable in their appended private-comment rows.
//! Reflow rebuilds these caches and cancels transient hover/delete targets.

use std::{collections::BTreeMap, ops::Range};

use ratatui::layout::Position;

use super::{AnnotationPosition, App};
use crate::reader::{self, Rendered};

#[derive(Default)]
pub(super) struct NoteMarks {
    pub(super) rows: BTreeMap<usize, Vec<i64>>,
    pub(super) passages: Vec<(Range<usize>, i64)>,
    pub(super) comments: Vec<(Range<usize>, i64)>,
    pub(super) hover: Option<(i64, Position)>,
    pub(super) delete: Option<(i64, i64)>,
}

impl App {
    pub(super) fn rebuild_note_marks(&mut self, rendered: &Rendered) {
        self.note_marks = NoteMarks::default();
        let source = reader::SourceIndex::new(&rendered.mappings);
        for entry in &self.annotations {
            let AnnotationPosition::Located { anchor, .. } = entry.position
            else {
                continue;
            };
            let note = &self.comments[entry.comment_index];
            let ranges = source.to_display(anchor..anchor + note.quote.len());
            if let Some(first) = ranges.first() {
                let row = rendered
                    .text_ranges
                    .partition_point(|row| row.end <= first.start);
                if row < rendered.lines.len() {
                    self.note_marks.rows.entry(row).or_default().push(note.id);
                }
            }
            self.note_marks
                .passages
                .extend(ranges.into_iter().map(|range| (range, note.id)));
        }
    }

    /// Annotation identities beginning on this wrapped reader row.
    pub fn reader_note_markers(&self, row: usize) -> &[i64] {
        self.note_marks.rows.get(&row).map_or(&[], Vec::as_slice)
    }

    /// Stable hover target and popup origin; no text parsing or storage reads.
    pub fn hovered_note(&self) -> Option<(i64, Position)> {
        self.note_marks.hover
    }

    pub(super) fn note_at_offset(&self, offset: usize) -> Option<i64> {
        let preferred =
            self.comments.get(self.comment_selected).map(|note| note.id);
        self.note_marks
            .passages
            .iter()
            .chain(&self.note_marks.comments)
            .filter(|(range, _)| range.contains(&offset))
            // A jump from an annotation card retains that note when quotes
            // overlap. An unrelated selection cannot override the cursor hit.
            .min_by_key(|(_, id)| Some(*id) != preferred)
            .map(|(_, id)| *id)
    }

    /// Note under the reader cursor or selected in the annotation panel.
    pub fn cursor_note(&self) -> Option<i64> {
        match self.focus {
            super::Focus::Annotations => {
                self.comments.get(self.comment_selected).map(|note| note.id)
            }
            super::Focus::Reader if !self.article_preparing() => {
                self.note_at_offset(self.cursor_offset())
            }
            _ => None,
        }
    }
}
