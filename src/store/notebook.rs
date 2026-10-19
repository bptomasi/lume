//! Independently indexed, paged annotations and their durable study workspace.
//!
//! Search predicates target one note. Metadata, links and user text share the
//! existing reversible annotation transaction. Draft checkpoints are separate
//! from committed text, and versions retain provenance through manual repair.

mod collections;
mod drafts;
mod export;
mod links;
mod metadata;
mod query;
mod review;
mod snippets;
mod versions;
mod writing;

pub(super) use drafts::clear_written_draft;
pub(super) use links::{replace_references, restore_references};
pub(super) use metadata::{read_metadata, restore_metadata};
