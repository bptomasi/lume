//! A selected note's preview and complete context share one annotation read.

use anyhow::{Context, Result};

use crate::{
    model::{
        AnnotationSnapshot, ContentSource, NoteDetail, NoteMetadata,
        NoteSummary,
    },
    store::Store,
};

struct Source {
    article: i64,
    title: String,
    publisher: String,
    url: String,
    feed_body: String,
}

impl Store {
    /// Load the selected note, references and source in one read transaction.
    ///
    /// # Errors
    /// Returns missing/trashed notes, corrupt metadata or SQLite errors.
    pub fn note_detail(&self, id: i64) -> Result<NoteDetail> {
        let transaction = self.connection.unchecked_transaction()?;
        let source = self.note_context_source(id)?;
        let note = self.annotation(source.article, id)?;
        let (body, source_available, source_revision) =
            if note.comment.source == ContentSource::Full {
                self.cached_document(source.article)?
                    .filter(|document| {
                        document.url == source.url && !document.body.is_empty()
                    })
                    .map_or_else(
                        || (String::new(), false, 0),
                        |document| (document.body, true, document.revision),
                    )
            } else {
                (source.feed_body, true, 0)
            };
        let detail = NoteDetail {
            summary: preview(&note, source.title, source.publisher),
            backlinks: self.note_backlinks(id)?,
            references: self.note_references(id)?,
            note,
            url: source.url,
            body,
            source_available,
            source_revision,
        };
        transaction.commit()?;
        Ok(detail)
    }

    fn note_context_source(&self, note: i64) -> Result<Source> {
        self.connection
            .query_row(
                include_str!("../sql/notebook/select_note_context.sql"),
                [note],
                |row| {
                    Ok(Source {
                        article: row.get(0)?,
                        title: row.get(1)?,
                        publisher: row.get(2)?,
                        url: row.get(3)?,
                        feed_body: row.get(4)?,
                    })
                },
            )
            .context("Note is missing or its article is in Trash")
    }
}

fn preview(
    note: &AnnotationSnapshot,
    title: String,
    source: String,
) -> NoteSummary {
    // List previews omit reference edges; the complete snapshot retains them.
    NoteSummary {
        id: note.comment.id,
        article: note.article,
        title,
        source,
        body: excerpt(&note.comment.body),
        preview: String::new(),
        quote: excerpt(&note.comment.quote),
        created: note.comment.created,
        metadata: NoteMetadata {
            kind: note.metadata.kind,
            resolved: note.metadata.resolved,
            pinned: note.metadata.pinned,
            updated_at: note.metadata.updated_at,
            references: Vec::new(),
        },
        tags: note.tags.clone(),
    }
}

fn excerpt(text: &str) -> String {
    // Match SQLite substr(..., 1, 512), including TEXT's NUL terminator.
    text.chars()
        .take_while(|character| *character != '\0')
        .take(512)
        .collect()
}
