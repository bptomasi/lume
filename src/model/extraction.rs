//! Captured rule previews and durable collection health.

use super::{ArticleTarget, ExtractedDocument};

/// Immutable preview request; workers cannot infer a target from current focus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RuleRequest {
    /// Requested article and observed URL.
    pub target: ArticleTarget,
    /// Captured subscription identity.
    pub feed: i64,
    /// Rule observed before starting the request.
    pub previous: String,
    /// Proposed CSS selector; empty requests the automatic extractor.
    pub selector: String,
}

/// Provenance reported by the feed parser, never guessed from text length.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FeedOrigin {
    /// Publisher content element; completeness is not guaranteed.
    #[default]
    Feed,
    /// Publisher summary element used as a fallback.
    Summary,
    /// No usable publisher content was supplied.
    Missing,
}

impl FeedOrigin {
    /// Stable database representation.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Feed => "feed",
            Self::Summary => "summary",
            Self::Missing => "missing",
        }
    }
}

/// Preview captured before confirmation; changing selection cannot retarget it.
#[derive(Clone, Debug)]
pub struct RulePreview {
    /// Requested article and its observed URL.
    pub target: ArticleTarget,
    /// Subscription that owns the rule.
    pub feed: i64,
    /// Rule observed before the preview, checked again on confirmation.
    pub previous: String,
    /// Validated proposed CSS selector; empty restores automatic extraction.
    pub selector: String,
    /// Sanitized result shown before saving either the rule or the cache.
    pub document: ExtractedDocument,
}

/// Bounded source diagnostics; timestamps are Unix seconds, zero means unknown.
#[derive(Clone, Debug)]
pub struct SourceHealth {
    /// Durable subscription identity.
    pub feed: i64,
    /// Terminal-safe source title.
    pub title: String,
    /// Original subscription URL, retained for diagnostics and hostname policy.
    pub url: String,
    /// Most recent collection attempt.
    pub attempted_at: i64,
    /// Most recent successful collection, including a 304 response.
    pub succeeded_at: i64,
    /// Earliest permitted retry following a collection failure.
    pub retry_at: i64,
    /// Consecutive collection failures.
    pub failures: u32,
    /// Last bounded collection error.
    pub error: String,
    /// Active extraction selector; empty means Readability.
    pub selector: String,
}
