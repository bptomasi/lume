//! Explicit targets and operations for personal organization.

use serde::{Deserialize, Serialize};

/// Independent tag owner; annotation tags never implicitly tag their article.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub enum TagTarget {
    /// Durable article identity.
    Article(i64),
    /// Durable comment/highlight identity.
    Comment(i64),
}

/// A single operation applied transactionally to captured article identities.
#[derive(Clone, Debug)]
pub enum BulkEdit {
    /// Explicit unread state.
    Unread(bool),
    /// Explicit favorite state.
    Favorite(bool),
    /// Explicit queue membership.
    Queue(bool),
    /// Add membership in an existing folder, preserving other memberships.
    Folder(i64),
    /// Add an independent article tag.
    Tag(String),
}
