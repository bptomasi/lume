//! Business operations share captured ownership with synchronous editor writes.

use anyhow::Result;

use crate::{
    model::{NoteDraft, TagTarget, UndoEffect},
    store::Store,
};

use super::{Edit, Outcome, Task};

pub(super) fn retry(store: &Store, task: Task) -> Result<Outcome> {
    let deadline =
        std::time::Instant::now() + std::time::Duration::from_secs(2);
    let mut pause = std::time::Duration::from_millis(2);
    loop {
        match perform(store, task.clone()) {
            Err(error)
                if busy(&error) && std::time::Instant::now() < deadline =>
            {
                // Every domain transaction rolls back before retrying. Read
                // failures after a commit remain inside the acknowledged
                // outcome, so a newly created identity can never be retried.
                std::thread::sleep(pause);
                pause = (pause * 2).min(std::time::Duration::from_millis(32));
            }
            result => return result,
        }
    }
}

fn busy(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        matches!(cause.downcast_ref::<rusqlite::Error>(),
            Some(rusqlite::Error::SqliteFailure(error, _))
                if matches!(error.code, rusqlite::ErrorCode::DatabaseBusy
                    | rusqlite::ErrorCode::DatabaseLocked))
    })
}

pub(super) fn perform(store: &Store, task: Task) -> Result<Outcome> {
    match task {
        Task::SavePage { document, visit } => Ok(Outcome::SavedPage {
            article: store.save_web_document(&document)?,
            visit,
        }),
        Task::Write(draft) => write(store, draft),
        Task::Query(edit) => query_edit(store, edit),
        Task::ExportArticles(path) => Ok(Outcome::Exported(
            crate::exchange::incremental::notebook(store, &path)?,
        )),
        Task::LinkArticle {
            article,
            note,
            related,
        } => {
            store.link_annotation(article, note, related)?;
            changed(store, UndoEffect::Notes(article))
        }
        Task::Export(path) => Ok(Outcome::Exported(
            crate::exchange::notes::notebook(store, &path)?,
        )),
        Task::Edit(edit) => {
            changed(store, UndoEffect::Notes(edit_note(store, edit)?))
        }
        Task::Articles { articles, edit } => {
            article_edit(store, articles, edit)
        }
        Task::Library(edit) => library_edit(store, edit),
        Task::Undo => changed(store, store.undo_effect()?),
        Task::Checkpoint(draft) => {
            store.save_note_draft(&draft)?;
            Ok(Outcome::Checkpoint)
        }
        Task::Discard(target) => {
            store.discard_note_draft(&target)?;
            Ok(Outcome::Checkpoint)
        }
    }
}

fn edit_note(store: &Store, edit: super::Edit) -> Result<i64> {
    match edit {
        Edit::Delete { article, note } => {
            store.delete_comment(article, note)?;
            Ok(article)
        }
        Edit::Review {
            target,
            days,
            postpone,
        } => super::review(store, &target, days, postpone),
        Edit::Metadata {
            article,
            note,
            change,
        } => {
            store.change_note_metadata(article, note, change)?;
            Ok(article)
        }
        Edit::Tags {
            article,
            note,
            tags,
        } => {
            store.annotation(article, note)?;
            store.set_tags(TagTarget::Comment(note), &tags)?;
            Ok(article)
        }
        Edit::RestoreVersion { current, version } => {
            store.restore_note_version(
                current.article,
                &current.comment,
                version,
            )?;
            Ok(current.article)
        }
        Edit::Reanchor { current, passage } => {
            store.reanchor_note(current.article, &current.comment, &passage)?;
            Ok(current.article)
        }
    }
}

fn changed(store: &Store, effect: UndoEffect) -> Result<Outcome> {
    let comments = if let UndoEffect::Notes(article) = effect {
        store.comments(article)
    } else {
        Ok(Vec::new())
    };
    Ok(Outcome::Changed { effect, comments })
}

fn write(store: &Store, draft: NoteDraft) -> Result<Outcome> {
    let target = store.write_note_draft(&draft)?;
    // A committed identity must survive a later presentation-read failure.
    let comments = store.comments(target.article());
    Ok(Outcome::Written {
        draft,
        target,
        comments,
    })
}

fn query_edit(store: &Store, edit: super::QueryEdit) -> Result<Outcome> {
    match edit {
        super::QueryEdit::Save { name, query } => {
            store.save_note_collection(&name, &query)?;
            Ok(Outcome::Collections)
        }
        super::QueryEdit::Delete(name) => {
            store.delete_note_collection(&name)?;
            Ok(Outcome::Collections)
        }
        super::QueryEdit::Remember(query) => {
            store.remember_note_search(&query)?;
            Ok(Outcome::SearchHistory(store.note_search_history()?))
        }
    }
}

pub(in crate::app) fn article_edit(
    store: &Store,
    articles: std::collections::BTreeSet<i64>,
    edit: crate::model::BulkEdit,
) -> Result<Outcome> {
    if articles.len() == 1 {
        for article in articles {
            match edit {
                crate::model::BulkEdit::Unread(value) => {
                    store.set_unread(article, value)?
                }
                crate::model::BulkEdit::Favorite(value) => {
                    store.set_starred(article, value)?
                }
                crate::model::BulkEdit::Queue(value) => {
                    store.set_queued(article, value)?
                }
                _ => store.edit_articles(&[article].into(), &edit)?,
            }
        }
    } else {
        store.edit_articles(&articles, &edit)?;
    }
    Ok(Outcome::Articles)
}

pub(in crate::app) fn library_edit(
    store: &Store,
    edit: super::LibraryEdit,
) -> Result<Outcome> {
    super::library::perform(store, edit)
}
