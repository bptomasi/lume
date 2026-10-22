//! Provenance and policy for feed text and independently fetched articles.

use serde::{Deserialize, Serialize};

/// Captured web content without a subscription or library mutation identity.
#[derive(Clone, Debug)]
pub struct WebDocument {
    /// Final validated HTTP(S) address.
    pub url: String,
    /// Terminal-safe display title.
    pub title: String,
    /// Original safe Markdown, retained on an explicit save.
    pub body: String,
}

/// Source of a body or captured passage; feed refreshes never replace Full.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum ContentSource {
    /// Publisher content delivered by a subscription.
    #[default]
    Feed,
    /// Independently extracted article page.
    Full,
}

impl ContentSource {
    /// Stable storage representation, independent of UI labels.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feed => "feed",
            Self::Full => "full",
        }
    }
}

/// Captured source passage; storage checks Full revisions before saving it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Passage {
    /// Immutable normalized source bytes selected by the reader.
    pub quote: String,
    /// UTF-8 byte anchor in that source body.
    pub anchor: usize,
    /// Origin of the selected text.
    pub source: ContentSource,
    /// Full-cache revision at capture, or zero for Feed.
    pub revision: i64,
}

/// Automatic fetching policy for one subscription; manual fetching is separate.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(rename_all = "lowercase")]
pub enum FetchPolicy {
    /// Fetch on opening an entry that contains no independent article text.
    #[default]
    Missing,
    /// Fetch on opening any uncached article, including summary-only entries.
    Always,
    /// Fetch only after an explicit request.
    Manual,
}

impl FetchPolicy {
    /// Stable storage representation and command argument.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Always => "always",
            Self::Manual => "manual",
        }
    }
}

/// Captured article identity and original URL, checked again before committing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArticleTarget {
    /// Durable local identity; independent of the currently selected row.
    pub article: i64,
    /// Article URL observed when the request was created.
    pub url: String,
}

/// Sanitized extraction result; HTTP and extraction run outside the UI thread.
#[derive(Clone, Debug)]
pub struct ExtractedDocument {
    /// Final HTTP(S) address after redirects, used as the link resolution base.
    pub final_url: String,
    /// Terminal-safe Markdown, limited to 512 KiB by storage.
    pub body: String,
}

/// A selected article's successful cache and most recent fetch outcome.
#[derive(Clone, Debug)]
pub struct CachedDocument {
    /// URL used for the successful extraction; may differ from the current URL.
    pub url: String,
    /// Resolved address of the successfully downloaded document.
    pub final_url: String,
    /// Extracted Markdown; empty when every attempt failed.
    pub body: String,
    /// Monotonic body revision; zero until a successful extraction.
    pub revision: i64,
    /// Successful extraction time in Unix seconds.
    pub fetched_at: i64,
    /// URL associated with the most recent success or failure.
    pub attempt_url: String,
    /// Most recent completion time in Unix seconds, used for retry cooldown.
    pub attempted_at: i64,
    /// Terminal-safe failure text, empty after success.
    pub error: String,
    /// Reading anchor for this revision of the full article.
    pub progress: usize,
}
