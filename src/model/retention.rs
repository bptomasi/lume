//! Counts shared by SQLite batches and retention orchestration.

/// Counts and logical bytes; reclaimed SQLite pages are reused, not vacuumed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Articles eligible for removal, or actually removed during application.
    pub articles: usize,
    /// Unprotected Full caches eligible, or actually cleared.
    pub full_texts: usize,
    /// Full body bytes released or eligible.
    pub full_bytes: u64,
    /// Feed body bytes released or eligible.
    pub feed_bytes: u64,
    /// Ordinary image files removed or eligible; pinned copies are untouched.
    pub images: usize,
    /// Ordinary image bytes removed or eligible.
    pub image_bytes: u64,
    /// Cancellation stopped the pass between batches.
    pub cancelled: bool,
    /// A concurrent library change interrupted the captured plan.
    pub interrupted: bool,
}

impl Report {
    pub(crate) fn add(&mut self, other: &Self) {
        self.articles += other.articles;
        self.full_texts += other.full_texts;
        self.full_bytes += other.full_bytes;
        self.feed_bytes += other.feed_bytes;
        self.images += other.images;
        self.image_bytes += other.image_bytes;
    }
}
