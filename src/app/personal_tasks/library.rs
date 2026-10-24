//! Captured article organization shares the ordered personal history lane.

use std::collections::BTreeSet;

use anyhow::Result;

use crate::{
    model::{TagTarget, UndoEffect},
    store::Store,
};

use super::{DeletionOutcome, Outcome};

#[derive(Clone)]
pub(in crate::app) enum Deletion {
    Trash { article: i64, deleted: bool },
    Purge(i64),
    CleanTrash,
    View(crate::model::View),
}

#[derive(Clone)]
pub(in crate::app) enum LibraryEdit {
    Delete(Deletion),
    Tags {
        target: TagTarget,
        tags: Vec<String>,
    },
    CreateFolder {
        name: String,
        article: Option<i64>,
    },
    RenameFolder {
        folder: i64,
        name: String,
    },
    DeleteFolder(i64),
    Unsubscribe(i64),
    RemoveServer(i64),
    DeleteSmart(i64),
    RemoveMember {
        article: i64,
        folder: i64,
    },
    Memberships {
        article: i64,
        folders: BTreeSet<i64>,
        names: BTreeSet<String>,
    },
}

pub(super) fn perform(store: &Store, edit: LibraryEdit) -> Result<Outcome> {
    let definition = edit.removed_view();
    match edit {
        LibraryEdit::Delete(edit) => return delete_articles(store, edit),
        LibraryEdit::Tags { target, tags } => {
            return tag_edit(store, target, tags);
        }
        LibraryEdit::CreateFolder { name, article } => {
            store.save_collection(&name, article)?;
        }
        LibraryEdit::RenameFolder { folder, name } => {
            store.rename_collection(folder, &name)?
        }
        LibraryEdit::DeleteFolder(folder) => store.delete_collection(folder)?,
        LibraryEdit::Unsubscribe(feed) => store.unsubscribe(feed)?,
        LibraryEdit::RemoveServer(id) => store.remove_server(id)?,
        LibraryEdit::DeleteSmart(id) => store.delete_smart_list(id)?,
        LibraryEdit::RemoveMember { article, folder } => {
            store.remove_member(folder, article)?
        }
        LibraryEdit::Memberships {
            article,
            folders,
            names,
        } => {
            store.set_named_memberships(article, &folders, &names)?;
        }
    }
    if let Some(view) = definition {
        return Ok(Outcome::DefinitionRemoved(view));
    }
    Ok(Outcome::Changed {
        effect: UndoEffect::Library,
        comments: Ok(Vec::new()),
    })
}

fn delete_articles(store: &Store, edit: Deletion) -> Result<Outcome> {
    let result = match edit {
        Deletion::Trash { article, deleted } => {
            // Reconcile stale metadata even if another connection set the flag.
            store.set_deleted(article, deleted)?;
            DeletionOutcome::Trash { article, deleted }
        }
        Deletion::Purge(article) => {
            store.purge_deleted(article)?;
            DeletionOutcome::Purged(article)
        }
        Deletion::CleanTrash => DeletionOutcome::Cleaned(store.clean_trash()?),
        Deletion::View(view) => DeletionOutcome::Trashed {
            count: store.trash_view(&view)?,
            view,
        },
    };
    Ok(Outcome::Deleted(result))
}

fn tag_edit(
    store: &Store,
    target: TagTarget,
    tags: Vec<String>,
) -> Result<Outcome> {
    store.set_tags(target, &tags)?;
    Ok(Outcome::Personal(match target {
        TagTarget::Article(_) => crate::search::Changes::TAGS,
        TagTarget::Comment(_) => crate::search::Changes::NOTES,
    }))
}

impl LibraryEdit {
    pub(in crate::app) fn removal_target(&self) -> Option<crate::model::View> {
        match self {
            Self::Delete(Deletion::View(view)) => Some(view.clone()),
            Self::Delete(Deletion::CleanTrash) => {
                Some(crate::model::View::Trash)
            }
            _ => self.removed_view(),
        }
    }

    fn removed_view(&self) -> Option<crate::model::View> {
        use crate::model::View;
        match self {
            Self::Unsubscribe(id) => Some(View::Feed(*id)),
            Self::RemoveServer(id) => Some(View::Server(*id)),
            Self::DeleteSmart(id) => Some(View::Smart(*id)),
            Self::DeleteFolder(id) => Some(View::Collection(*id)),
            _ => None,
        }
    }
}
