//! Captured article organization shares the ordered personal history lane.

use std::collections::BTreeSet;

use anyhow::Result;

use crate::{
    model::{TagTarget, UndoEffect},
    store::Store,
};

use super::Outcome;

#[derive(Clone)]
pub(in crate::app) enum LibraryEdit {
    Deleted {
        article: i64,
        deleted: bool,
    },
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
    let definition = matches!(
        &edit,
        LibraryEdit::Unsubscribe(_) | LibraryEdit::DeleteSmart(_)
    );
    match edit {
        LibraryEdit::Deleted { article, deleted } => {
            store.set_deleted(article, deleted)?;
        }
        LibraryEdit::Tags { target, tags } => store.set_tags(target, &tags)?,
        LibraryEdit::CreateFolder { name, article } => {
            store.save_collection(&name, article)?;
        }
        LibraryEdit::RenameFolder { folder, name } => {
            store.rename_collection(folder, &name)?
        }
        LibraryEdit::DeleteFolder(folder) => store.delete_collection(folder)?,
        LibraryEdit::Unsubscribe(feed) => store.unsubscribe(feed)?,
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
    if definition {
        return Ok(Outcome::Definitions);
    }
    Ok(Outcome::Changed {
        effect: UndoEffect::Library,
        comments: Ok(Vec::new()),
    })
}
