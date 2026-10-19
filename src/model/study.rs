//! User-authored study material and explicit review dates, without inference.

use serde::{Deserialize, Serialize};

use super::Comment;

/// A user-authored question and answer attached to an annotation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyCard {
    /// Durable card identity.
    pub id: i64,
    /// Owning annotation.
    pub comment: i64,
    /// User-authored question, at most 1 KiB.
    pub question: String,
    /// User-authored answer, at most 8 KiB.
    pub answer: String,
    /// Next review, Unix seconds; zero means immediately available.
    pub due_at: i64,
    /// Explicitly chosen review interval, in days.
    pub interval_days: u16,
    /// Completed review count.
    pub reviews: u32,
}

/// Explicit periodic review state for an immutable saved passage.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewSchedule {
    /// Next review, Unix seconds.
    pub due_at: i64,
    /// User-chosen interval, in days.
    pub interval_days: u16,
    /// Completed reviews.
    pub reviews: u32,
}

/// Complete private annotation snapshot, including dependent study material.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AnnotationSnapshot {
    /// Owning article identity.
    pub article: i64,
    /// Original note and quote, including provenance.
    pub comment: Comment,
    /// Independent highlight tags.
    pub tags: Vec<String>,
    /// Related article identities.
    pub links: Vec<i64>,
    /// Optional explicit passage-review schedule.
    pub review: Option<ReviewSchedule>,
    /// User-authored cards retained across deletion/undo.
    pub cards: Vec<StudyCard>,
    /// Notebook metadata and references, absent in legacy undo records.
    #[serde(default)]
    pub metadata: super::NoteMetadata,
}
