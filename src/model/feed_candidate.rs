//! Explicit subscription choices shared by discovery and durable ingestion.

/// An explicit discovered subscription. Discovery never subscribes silently.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FeedCandidate {
    /// Publisher label, or normalized URL when absent.
    pub title: String,
    /// Validated absolute HTTP(S) URL.
    pub url: String,
}
