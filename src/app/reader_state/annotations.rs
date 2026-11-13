//! Annotation ordering belongs to the source it describes.
use super::ReaderState;
use crate::app::{AnnotationEntry, AnnotationPosition};
use crate::reader::locate_quote;

impl ReaderState {
    pub(in crate::app) fn select_first_annotation(&mut self) {
        if let Some(entry) = self.notes.entries.first() {
            self.notes.selected = entry.comment_index;
        }
    }

    pub(super) fn rebuild_annotations(&mut self) {
        let mut relocated = std::collections::HashMap::new();
        self.notes.entries = self
            .notes
            .comments
            .iter()
            .enumerate()
            .map(|(index, note)| AnnotationEntry {
                id: note.id,
                comment_index: index,
                position: self.annotation_position(note, &mut relocated),
            })
            .collect();
        self.notes.entries.sort_by_key(AnnotationEntry::order);
    }

    pub(super) fn annotation_position<'a>(
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
        if note.source != self.publisher.source {
            return AnnotationPosition::OtherSource {
                anchor: note.anchor,
            };
        }
        let anchor = if note.source_revision == self.publisher.revision {
            note.anchor
        } else {
            self.publisher.body.len()
        };
        let exact = self
            .publisher
            .body
            .get(anchor..)
            .is_some_and(|tail| tail.starts_with(&note.quote));
        let range = if exact {
            Some(anchor..anchor + note.quote.len())
        } else {
            relocated
                .entry(note.quote.as_str())
                .or_insert_with(|| {
                    locate_quote(
                        &self.publisher.body,
                        &note.quote,
                        self.publisher.body.len(),
                    )
                })
                .clone()
        };
        match range {
            Some(range) => AnnotationPosition::Located {
                anchor: range.start,
                percent: range.start.saturating_mul(100)
                    / self.publisher.body.len().max(1),
            },
            None => AnnotationPosition::Unplaced,
        }
    }

    pub(in crate::app) fn annotation_selected(&self) -> Option<usize> {
        if self.notes.entries.is_empty() {
            return None;
        }
        Some(
            self.notes
                .entries
                .iter()
                .position(|entry| entry.comment_index == self.notes.selected)
                .unwrap_or(0),
        )
    }

    pub(in crate::app) fn move_annotation(
        &mut self,
        forward: bool,
        distance: usize,
    ) {
        let Some(selected) = self.annotation_selected() else {
            return;
        };
        let next = if forward {
            selected
                .saturating_add(distance)
                .min(self.notes.entries.len() - 1)
        } else {
            selected.saturating_sub(distance)
        };
        self.notes.selected = self.notes.entries[next].comment_index;
    }
}

impl ReaderState {
    pub(in crate::app) fn reveal_selected_comment(&mut self) {
        let marker = format!("[{}] ", self.notes.selected + 1);
        if let Some(row) = self
            .projection
            .lines
            .iter()
            .position(|line| line.starts_with(&marker))
        {
            self.point_cursor(row, 0);
            self.ensure_cursor_visible();
        }
    }
}
