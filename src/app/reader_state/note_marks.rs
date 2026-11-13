//! Source-to-display note correspondence, rebuilt with each projection.
use crate::app::AnnotationPosition;
use ratatui::layout::Position;
use std::{collections::BTreeMap, ops::Range};
#[derive(Default)]
pub(super) struct NoteMarks {
    pub(super) rows: BTreeMap<usize, Vec<i64>>,
    pub(super) passages: Vec<(Range<usize>, i64)>,
    pub(super) comments: Vec<(Range<usize>, i64)>,
    pub(super) hover: Option<(i64, Position)>,
    pub(super) delete: Option<(i64, i64)>,
}

use super::ReaderState;
use crate::reader::{self, Rendered};

impl ReaderState {
    pub(super) fn rebuild_note_marks(&mut self, rendered: &Rendered) {
        self.notes.marks = NoteMarks::default();
        let source = reader::SourceIndex::new(&rendered.mappings);
        for entry in &self.notes.entries {
            let AnnotationPosition::Located { anchor, .. } = entry.position
            else {
                continue;
            };
            let note = &self.notes.comments[entry.comment_index];
            let ranges = source.to_display(anchor..anchor + note.quote.len());
            if let Some(first) = ranges.first() {
                let row = rendered
                    .text_ranges
                    .partition_point(|row| row.end <= first.start);
                if row < rendered.lines.len() {
                    self.notes.marks.rows.entry(row).or_default().push(note.id);
                }
            }
            self.notes
                .marks
                .passages
                .extend(ranges.into_iter().map(|range| (range, note.id)));
        }
    }

    pub(in crate::app) fn reader_note_markers(&self, row: usize) -> &[i64] {
        self.notes.marks.rows.get(&row).map_or(&[], Vec::as_slice)
    }

    pub(in crate::app) fn hovered_note(&self) -> Option<(i64, Position)> {
        self.notes.marks.hover
    }

    pub(in crate::app) fn note_at_offset(&self, offset: usize) -> Option<i64> {
        let preferred = self
            .notes
            .comments
            .get(self.notes.selected)
            .map(|note| note.id);
        note_at_offset(
            &self.notes.marks.passages,
            &self.notes.marks.comments,
            preferred,
            offset,
        )
    }
}

impl ReaderState {
    pub(in crate::app) fn pending_note_delete(&self) -> Option<(i64, i64)> {
        self.notes.marks.delete
    }

    pub(in crate::app) fn take_note_delete(&mut self) -> Option<(i64, i64)> {
        self.notes.marks.delete.take()
    }

    pub(in crate::app) fn arm_note_delete(
        &mut self,
        target: Option<(i64, i64)>,
    ) {
        self.notes.marks.delete = target;
    }

    pub(in crate::app) fn hover_note(
        &mut self,
        target: Option<(i64, Position)>,
    ) {
        self.notes.marks.hover = target;
    }
}

/// Resolve the selected annotation consistently when quoted passages overlap.
pub(super) fn note_at_offset(
    passages: &[(Range<usize>, i64)],
    comments: &[(Range<usize>, i64)],
    preferred: Option<i64>,
    offset: usize,
) -> Option<i64> {
    passages
        .iter()
        .chain(comments)
        .filter(|(range, _)| range.contains(&offset))
        // A jump from an annotation card retains that note when quotes
        // overlap. An unrelated selection cannot override the cursor hit.
        .min_by_key(|(_, id)| Some(*id) != preferred)
        .map(|(_, id)| *id)
}
