//! Storage-owned article data crosses the worker boundary as one typed snapshot.

use anyhow::Result;

use crate::{
    model::{CachedDocument, Comment, FeedOrigin, FetchPolicy},
    store::Store,
};

pub(super) struct ReadingData {
    pub(super) body: String,
    pub(super) origin: FeedOrigin,
    pub(super) document: Option<CachedDocument>,
    pub(super) policy: FetchPolicy,
    pub(super) comments: Vec<Comment>,
    pub(super) progress: usize,
}

impl ReadingData {
    pub(super) fn load(store: &Store, id: i64, feed: i64) -> Result<Self> {
        Ok(Self {
            body: store.body(id)?,
            origin: store.feed_origin(id)?,
            document: store.cached_document(id)?,
            policy: store.fetch_policy(feed)?,
            comments: store.comments(id)?,
            progress: store.progress(id)?,
        })
    }
}
