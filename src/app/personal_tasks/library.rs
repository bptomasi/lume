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
    let definition = edit.is_definition();
    match edit {
        LibraryEdit::Deleted { article, deleted } => {
            return delete_article(store, article, deleted);
        }
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
    if definition {
        return Ok(Outcome::Definitions);
    }
    Ok(Outcome::Changed {
        effect: UndoEffect::Library,
        comments: Ok(Vec::new()),
    })
}

fn delete_article(
    store: &Store,
    article: i64,
    deleted: bool,
) -> Result<Outcome> {
    // Another connection may already have set this flag while our metadata
    // still shows the row. Confirm durable state and reconcile it either way.
    store.set_deleted(article, deleted)?;
    Ok(Outcome::Deleted { article, deleted })
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
    fn is_definition(&self) -> bool {
        matches!(
            self,
            Self::Unsubscribe(_) | Self::RemoveServer(_) | Self::DeleteSmart(_)
        )
    }
}
