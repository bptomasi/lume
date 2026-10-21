//! Local recommendation preferences and evidence; scores are not quality ratings.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// An explicit interest expressed using Lume's bounded boolean search grammar.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Interest {
    /// Stable, case-sensitive name used in explanations and feedback.
    pub name: String,
    /// Priority from one through ten.
    pub weight: u8,
    /// Terms, phrases and boolean relations, evaluated by SQLite FTS5.
    pub query: String,
}

/// Portable local preferences, independent of a model or hosted account.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RecommendationProfile {
    /// At most 24 interests; an empty profile deliberately recommends nothing.
    pub interests: Vec<Interest>,
    /// Optional exclusion query, using the same grammar as interests.
    pub exclude: String,
    /// Maximum estimated reading time; absent means unrestricted.
    pub minutes: Option<u16>,
    /// Include articles explicitly marked read.
    pub include_read: bool,
    /// Explicit subscription preferences from minus ten through ten.
    pub sources: BTreeMap<i64, i8>,
}

/// Explicit feedback; reading an article never implicitly approves it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RecommendationFeedback {
    /// Useful enough to increase affinity for its matching interests.
    Useful,
    /// Hide this article and reduce its matching interest affinities.
    NotInterested,
    /// Hide this article without concluding that its subject is unwanted.
    TooBasic,
    /// Hide this article without concluding that its subject is unwanted.
    TooAdvanced,
}

/// Persisted evidence captured when this article was last classified.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RecommendationEvidence {
    /// Explicit interests matching the publisher index, never private notes.
    pub interests: Vec<String>,
    /// Matching interests whose query also matches the title alone.
    pub headline: Vec<String>,
    /// Feed, summary, title only, or feed plus matching cached full article.
    pub evaluated_from: String,
    /// Fenced code was observed in the available publisher text.
    pub code: bool,
    /// Title contains explicit announcement/webinar/sponsored wording.
    pub announcement: bool,
    /// Number of available words, capped by ingestion's document limit.
    pub words: usize,
}

/// One bounded incremental classification/publication operation.
#[derive(Debug)]
pub struct RecommendationUpdate {
    /// Articles whose cached classification or score was refreshed.
    pub processed: usize,
    /// Whether another bounded batch remains in the durable dirty queue.
    pub remaining: bool,
    /// Number of published recommendations, at most 100.
    pub total: usize,
}

impl RecommendationFeedback {
    /// Stable storage spelling, independent of a presentation label.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Useful => "useful",
            Self::NotInterested => "not-interested",
            Self::TooBasic => "too-basic",
            Self::TooAdvanced => "too-advanced",
        }
    }
}
