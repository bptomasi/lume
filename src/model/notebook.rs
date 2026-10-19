//! Annotation identity, independent metadata and recoverable editor contracts.

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::{Comment, Passage};

/// User-chosen meaning of a note; publisher content never assigns it.
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum NoteKind {
    /// A useful observation.
    #[default]
    Insight,
    /// An unanswered or resolved study question.
    Question,
    /// A concept explained in the user's words.
    Definition,
    /// A concrete implementation or production example.
    Example,
    /// A follow-up action.
    Todo,
    /// A synthesis referencing several source notes.
    Synthesis,
}

impl NoteKind {
    /// Supported kinds in picker order.
    pub const ALL: [Self; 6] = [
        Self::Insight,
        Self::Question,
        Self::Definition,
        Self::Example,
        Self::Todo,
        Self::Synthesis,
    ];

    /// Stable storage and query spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::Insight => "insight",
            Self::Question => "question",
            Self::Definition => "definition",
            Self::Example => "example",
            Self::Todo => "todo",
            Self::Synthesis => "synthesis",
        }
    }

    /// Resolve an explicit kind without silently replacing invalid input.
    ///
    /// # Errors
    /// Returns an unknown kind.
    pub fn named(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.name() == name)
            .ok_or_else(|| anyhow::anyhow!("Unknown note type: {name}"))
    }
}

/// Metadata and note-to-note edges retained with an annotation's undo record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteMetadata {
    /// Explicit user-selected meaning.
    pub kind: NoteKind,
    /// Whether the question/action has been resolved.
    pub resolved: bool,
    /// Whether the note is pinned above ordinary results.
    pub pinned: bool,
    /// Unix seconds of the latest text or metadata change.
    pub updated_at: i64,
    /// Incoming and outgoing edges, as (source, target) identities.
    pub references: Vec<(i64, i64)>,
}

/// One independently searchable annotation, bounded to a result preview.
#[derive(Clone, Debug)]
pub struct NoteSummary {
    /// Durable note identity, never reused after deletion.
    pub id: i64,
    /// Owning source article.
    pub article: i64,
    /// Article headline.
    pub title: String,
    /// Publisher/source label.
    pub source: String,
    /// At most 512 characters of user text.
    pub body: String,
    /// FTS excerpt; U+001E/U+001F delimit query matches for styled painting.
    pub preview: String,
    /// At most 512 characters of the original quotation.
    pub quote: String,
    /// Annotation creation time, independent of article publication.
    pub created: i64,
    /// Independent kind, status and pin state.
    pub metadata: NoteMetadata,
    /// Independent annotation tags.
    pub tags: Vec<String>,
}

/// A bounded note search page; all criteria match the same annotation.
#[derive(Clone, Debug, Default)]
pub struct NotePage {
    /// Total matching annotations in the transaction snapshot.
    pub total: usize,
    /// Bounded result window.
    pub notes: Vec<NoteSummary>,
}

/// One coherent note/source snapshot, loaded only for the selected preview.
pub struct NoteDetail {
    /// Bounded result metadata.
    pub summary: NoteSummary,
    /// Full selected note and study state.
    pub note: super::AnnotationSnapshot,
    /// Original URL.
    pub url: String,
    /// Publisher body, bounded by the existing source storage contract.
    pub body: String,
    /// Whether the captured Feed/Full source is still available for placement.
    pub source_available: bool,
    /// Current stored revision; changed revisions require unique quote relocation.
    pub source_revision: i64,
    /// Active incoming references.
    pub backlinks: Vec<NoteSummary>,
    /// Active outgoing references.
    pub references: Vec<NoteSummary>,
}

/// A saved dynamic collection shares the live note-query grammar.
#[derive(Clone, Debug)]
pub struct NoteCollection {
    /// Durable collection identity.
    pub id: i64,
    /// User-facing English or user-authored name.
    pub name: String,
    /// Validated note search expression.
    pub query: String,
}

/// A persistent, bounded version of note text and its source anchor.
#[derive(Clone, Debug)]
pub struct NoteVersion {
    /// Version identity, distinct from note identity.
    pub id: i64,
    /// Owning article identity.
    pub article: i64,
    /// The saved text, quotation and provenance.
    pub comment: Comment,
    /// Unix seconds when this version was recorded.
    pub saved_at: i64,
}

/// An editor captures ownership before refresh, selection or recovery.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DraftTarget {
    /// New or existing general/passage note.
    Note {
        /// Captured source article.
        article: i64,
        /// Existing note, or a new general note.
        note: Option<i64>,
    },
    /// A newly captured passage with immutable source evidence.
    Passage {
        /// Captured source article.
        article: i64,
        /// Source bytes and revision at capture.
        passage: Passage,
    },
    /// A user-authored question and answer on a captured note.
    Card {
        /// Captured source article.
        article: i64,
        /// Captured annotation.
        note: i64,
        /// Existing card after a successful first write.
        card: Option<i64>,
    },
}

impl DraftTarget {
    /// Stable buffer key. Source evidence is also checked before overwriting.
    pub fn key(&self) -> String {
        match self {
            Self::Note { article, note } => format!("note:{article}:{note:?}"),
            Self::Passage { article, passage } => format!(
                "passage:{article}:{}:{}:{}",
                passage.source.as_str(),
                passage.revision,
                passage.anchor,
            ),
            Self::Card {
                article,
                note,
                card,
            } => format!("card:{article}:{note}:{card:?}"),
        }
    }

    /// Owning article used to reject missing/trashed recovery targets.
    pub fn article(&self) -> i64 {
        match self {
            Self::Note { article, .. }
            | Self::Passage { article, .. }
            | Self::Card { article, .. } => *article,
        }
    }
}

/// Recoverable user text; autosave is separate from committing a note.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteDraft {
    /// Captured note/card/source target.
    pub target: DraftTarget,
    /// Current editor text, at most 16 KiB.
    pub body: String,
    /// Last committed text, for a correct modified-buffer guard after recovery.
    pub baseline: String,
    /// Optional type assigned atomically when a new note is first saved.
    #[serde(default)]
    pub initial_kind: Option<NoteKind>,
}

/// Explicit review grades choose transparent intervals, not an inferred model.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewGrade {
    /// Revisit tomorrow.
    Again,
    /// Revisit in three days.
    Hard,
    /// Revisit in seven days.
    Good,
    /// Revisit in fourteen days.
    Easy,
}

impl ReviewGrade {
    /// User-visible interval in days.
    pub fn days(self) -> u16 {
        match self {
            Self::Again => 1,
            Self::Hard => 3,
            Self::Good => 7,
            Self::Easy => 14,
        }
    }

    /// Resolve a grade from its documented command spelling.
    ///
    /// # Errors
    /// Returns an unsupported grade.
    pub fn named(name: &str) -> Result<Self> {
        Ok(match name {
            "again" => Self::Again,
            "hard" => Self::Hard,
            "good" => Self::Good,
            "easy" => Self::Easy,
            _ => bail!("Use again, hard, good or easy"),
        })
    }
}

/// A coherent export snapshot without loading the publisher's source body.
pub struct NoteExport {
    /// Complete private annotation.
    pub note: super::AnnotationSnapshot,
    /// Source citation URL.
    pub url: String,
    /// Bounded active incoming references.
    pub backlinks: Vec<NoteSummary>,
}

/// One property change resolved against the current transactional note state.
#[derive(Clone, Copy, Debug)]
pub enum NoteChange {
    /// Assign a user-chosen meaning.
    Kind(NoteKind),
    /// Set an explicit resolution state.
    Resolved(bool),
    /// Toggle the current resolution state, including queued rapid presses.
    ToggleResolved,
    /// Toggle the current pin state, including queued rapid presses.
    TogglePin,
}
