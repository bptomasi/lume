//! Stable pane identities shared by persisted layout, input and geometry.

use serde::{Deserialize, Serialize};

/// An independently focusable, collapsible workspace pane.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pane {
    /// Built-in destinations, folders and subscriptions.
    Library,
    /// Article headlines.
    Articles,
    /// Article text and precise visual selection.
    Reader,
    /// Article-local notes in source order.
    Annotations,
}

impl Pane {
    /// Complete default order, including the initially collapsed notes pane.
    pub const ALL: [Self; 4] = [
        Self::Library,
        Self::Articles,
        Self::Reader,
        Self::Annotations,
    ];

    /// Stable command spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::Library => "library",
            Self::Articles => "articles",
            Self::Reader => "reader",
            Self::Annotations => "annotations",
        }
    }

    /// English title used in pane headers and restoration buttons.
    pub fn title(self) -> &'static str {
        match self {
            Self::Library => "Library",
            Self::Articles => "Articles",
            Self::Reader => "Reader",
            Self::Annotations => "Annotations",
        }
    }

    /// Resolve a documented pane name without accepting arbitrary identifiers.
    pub fn named(name: &str) -> Option<Self> {
        if name == "notes" {
            return Some(Self::Annotations);
        }
        Self::ALL.into_iter().find(|pane| pane.name() == name)
    }
}
