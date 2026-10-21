//! Notebook actions capture the selected note instead of the reader's article.

use anyhow::{Context, Result};

use super::super::{App, InputKind, ReaderLocation};

impl App {
    pub(in crate::app) fn edit_notebook_note(&mut self) -> Result<()> {
        let context = self
            .notebook
            .as_ref()
            .and_then(|notebook| notebook.current_context())
            .context("Wait for the selected note's context")?;
        let article = context.note.article;
        let comment = &context.note.comment;
        self.ensure_no_recovery(&crate::model::DraftTarget::Note {
            article,
            note: Some(comment.id),
        })?;
        self.begin_input(
            InputKind::Comment {
                article,
                comment: Some(comment.id),
            },
            comment.body.clone(),
        );
        Ok(())
    }

    pub(super) fn notebook_source(&mut self) -> Result<()> {
        let context = self
            .notebook
            .as_ref()
            .and_then(|notebook| notebook.current_context())
            .context("Wait for the selected note's context")?;
        let note = &context.note;
        let location = ReaderLocation {
            article: note.article,
            source: note.comment.source,
            revision: note.comment.source_revision,
            anchor: note.comment.anchor,
            quote: (!note.comment.quote.is_empty())
                .then(|| note.comment.quote.clone()),
        };
        let id = note.comment.id;
        self.suspend_notebook_for_source();
        self.jump_reader(location)?;
        if let Some(index) = self.comments.iter().position(|note| note.id == id)
        {
            self.comment_selected = index;
        }
        Ok(())
    }

    pub(in crate::app) fn set_notebook_query(
        &mut self,
        text: &str,
    ) -> Result<()> {
        let notebook = self.notebook.as_mut().context("Open Notebook first")?;
        if notebook.query == text {
            return Ok(());
        }
        crate::search::notes::validate(text)?;
        notebook.query = text.into();
        notebook.offset = 0;
        notebook.selected = 0;
        notebook.refresh()
    }
}
